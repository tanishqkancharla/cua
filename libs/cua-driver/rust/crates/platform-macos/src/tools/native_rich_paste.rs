//! Prepare rich Mac clipboard data before touching any clipboard or target.
//! Markdown uses CommonMark; AppKit imports HTML and exports RTF. Publishing
//! the resulting RTF and its own string keeps the insertion oracle consistent
//! with the rich representation rather than guessing text from HTML tags.

use objc2::{
    rc::{autoreleasepool, Retained},
    runtime::AnyObject,
    ClassType,
};
use objc2_app_kit::{
    NSAttributedStringDocumentFormats, NSDocumentTypeDocumentAttribute, NSHTMLTextDocumentType,
    NSRTFTextDocumentType,
};
use objc2_foundation::{
    MainThreadMarker, NSAttributedString, NSData, NSDictionary, NSRange, NSString,
};
use std::{ffi::c_void, sync::mpsc, time::Duration};

#[derive(Clone)]
pub(super) struct PastePayload {
    pub text: String,
    pub rtf: Option<Vec<u8>>,
}

pub(super) fn prepare(source: &str, format: &str) -> Result<PastePayload, String> {
    if format == "text" {
        return Ok(PastePayload {
            text: source.to_owned(),
            rtf: None,
        });
    }
    let html = match format {
        "html" => source.to_owned(),
        "md" => {
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(source));
            html
        }
        _ => return Err("Supported Mac paste formats are text, html and md (CommonMark).".into()),
    };
    if MainThreadMarker::new().is_some() {
        return import_html(&html);
    }
    // HTML import uses AppKit's main run loop. The queued job only converts
    // data: even if the bounded wait expires, it cannot paste or write a board.
    let (tx, rx) = mpsc::sync_channel(1);
    let request = Box::new(ImportRequest { html, tx });
    unsafe {
        dispatch_async_f(
            &raw const _dispatch_main_q as *const c_void,
            Box::into_raw(request).cast(),
            import_on_main,
        );
    }
    rx.recv_timeout(Duration::from_secs(5)).map_err(|_| {
        "Rich paste conversion timed out before clipboard or input mutation.".to_owned()
    })?
}

struct ImportRequest {
    html: String,
    tx: mpsc::SyncSender<Result<PastePayload, String>>,
}

unsafe extern "C" fn import_on_main(context: *mut c_void) {
    let request = unsafe { Box::from_raw(context.cast::<ImportRequest>()) };
    let _ = request.tx.send(import_html(&request.html));
}

#[link(name = "System")]
extern "C" {
    static _dispatch_main_q: u8;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        function: unsafe extern "C" fn(*mut c_void),
    );
}

fn import_html(html: &str) -> Result<PastePayload, String> {
    if MainThreadMarker::new().is_none() {
        return Err("Rich paste conversion requires AppKit's main thread.".into());
    }
    autoreleasepool(|_| unsafe {
        let document =
            format!("<html><head><meta charset=\"UTF-8\"></head><body>{html}</body></html>");
        let data = NSData::with_bytes(document.as_bytes());
        let options = NSDictionary::from_id_slice(
            &[NSDocumentTypeDocumentAttribute],
            &[NSString::from_str(&NSHTMLTextDocumentType.to_string())],
        );
        // Objective-C dictionaries erase value generics. NSString is an
        // AnyObject; the retained container remains immutable here.
        let options: Retained<NSDictionary<NSString, AnyObject>> = Retained::cast(options);
        let attributed = NSAttributedString::initWithData_options_documentAttributes_error(
            NSAttributedString::alloc(),
            &data,
            &options,
            None,
        )
        .map_err(|error| format!("HTML import refused before clipboard mutation: {error}"))?;
        let text = attributed.string().to_string();
        let attributes = NSDictionary::from_id_slice(
            &[NSDocumentTypeDocumentAttribute],
            &[NSString::from_str(&NSRTFTextDocumentType.to_string())],
        );
        let attributes: Retained<NSDictionary<NSString, AnyObject>> = Retained::cast(attributes);
        let rtf = attributed
            .dataFromRange_documentAttributes_error(
                NSRange::new(0, attributed.length()),
                &attributes,
            )
            .map_err(|error| format!("RTF export refused before clipboard mutation: {error}"))?;
        if text.len() > 64 * 1024 || rtf.len() > 256 * 1024 {
            return Err(
                "Converted rich paste exceeds its bounded payload budget; no mutation occurred."
                    .into(),
            );
        }
        Ok(PastePayload {
            text,
            rtf: Some(rtf.bytes().to_vec()),
        })
    })
}
