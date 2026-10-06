# Chrome app-root accessibility enablement

Refs https://github.com/tanishqkancharla/opensky/issues/16

Maintainer-selected Mac parity investigation. Depends on OpenSky Driver PR26
(which includes upgrade PR25). Root AXRole query alone restores page controls
in an independently owned cold process, without native-provider or input use.
Read root role during process-lifetime enablement before existing opt-in/wait;
retain exact-window ownership, typed tokens and materialization bounds.

Draft before runtime edit. Existing GUI regressions, cold body/action oracle and
fresh affected serial Terra comparisons pending. No platform matrix claim.
