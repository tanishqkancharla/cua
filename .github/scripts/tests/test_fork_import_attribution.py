from copy import deepcopy
import pytest
from release_attribution import ReleaseError, verified_fork_import_policy, validate_pr_attribution
UPSTREAM = "a" * 40
HEAD = "b" * 40
TRUSTED = "c" * 40
BODY = f"Upstream sync: https://github.com/upstream/repo/commit/{UPSTREAM}"
class API:
    def __init__(self):
        self.fork=True; self.parent="upstream/repo"; self.ancestor=UPSTREAM
        self.commit_sha=UPSTREAM; self.mapping={"inherited@example.com":"original-author"}
        self.batch=[{"sha":UPSTREAM}]; self.total=1
    def get(self,path):
        if path=="repos/fork/repo": return {"fork":self.fork,"parent":{"full_name":self.parent}}
        if path==f"repos/upstream/repo/commits/{UPSTREAM}": return {"sha":self.commit_sha}
        assert path==f"repos/fork/repo/compare/{UPSTREAM}...{HEAD}?per_page=1"
        return {"merge_base_commit":{"sha":self.ancestor}}
    def file_json(self,repo,path,ref):
        assert (repo,path,ref)==("upstream/repo",".github/release-attribution-config.json",UPSTREAM)
        return {"identityOverrides":self.mapping}
    def compare_commits(self,repo,base,head):
        assert (repo,base,head)==("fork/repo",TRUSTED,UPSTREAM)
        return self.batch,self.total

def resolve(api=None,body=BODY,changes=None):
    base={"identityOverrides":{"trusted@example.com":"trusted-author"}}; before=deepcopy(base)
    result=verified_fork_import_policy(repository="fork/repo",pull={"body":body},
        head_sha=HEAD,trusted_sha=TRUSTED,config_path=".github/release-attribution-config.json",
        base_config=base,identity_changes=changes or {"inherited@example.com":"original-author"},
        commits=[{"sha":UPSTREAM},{"sha":HEAD}],github=api or API())
    assert base==before
    return result

def test_verified_import_only_exempts_original_commits():
    policy,commits=resolve()
    assert policy["identityOverrides"]["inherited@example.com"]=="original-author"
    assert commits==[{"sha":HEAD}]

def test_no_marker_keeps_existing_contract():
    policy,commits=resolve(body="ordinary PR")
    assert "inherited@example.com" not in policy["identityOverrides"]
    assert len(commits)==2

@pytest.mark.parametrize("field,value,error",[
    ("fork",False,"verified GitHub fork parent"),
    ("parent","attacker/repo","verified parent"),
    ("ancestor",HEAD,"not an ancestor"),
    ("commit_sha",HEAD,"could not be verified"),
    ("total",2,"incomplete commit provenance"),
])
def test_unverified_import_refuses(field,value,error):
    api=API();setattr(api,field,value)
    with pytest.raises(ReleaseError,match=error):resolve(api)

def test_wrong_login_requires_existing_source_proof():
    policy,_=resolve(changes={"inherited@example.com":"attacker"})
    with pytest.raises(ReleaseError,match="no explicit same-repository source PR"):
        validate_pr_attribution(repository="fork/repo",pull={"number":1},commits=[],
            base_config=policy,identity_changes={"inherited@example.com":"attacker"},github=API())

@pytest.mark.parametrize("change",[None,"attacker"])
def test_import_cannot_remove_or_replace_trusted_identity(change):
    api=API();api.mapping["trusted@example.com"]="attacker"
    policy,_=resolve(api,changes={"trusted@example.com":change})
    with pytest.raises(ReleaseError,match="removes or changes trusted identityOverrides"):
        validate_pr_attribution(repository="fork/repo",pull={"number":1},commits=[],
            base_config=policy,identity_changes={"trusted@example.com":change},github=api)
