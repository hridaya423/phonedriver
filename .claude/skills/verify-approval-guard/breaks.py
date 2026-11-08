import subprocess, sys, os

OUT = "target/verify-approval-guard"
os.makedirs(OUT, exist_ok=True)
BREAKS = {
 "B1-approval-on-every-batch-step": ("crates/cli/src/daemon/actor.rs", "approval: p.approval.filter(|a| *a == Approval::Autonomous || i == 0),", "approval: p.approval,"),
 "B2-resume-from-step-zero": ("crates/cli/src/mcp.rs", "            from = held;\n        }\n    }", "            from = 0;\n        }\n    }"),
 "B2b-mrtr-retry-resumes-from-step-zero": ("crates/cli/src/mcp.rs", "            from = step;\n        }", "            from = 0;\n        }"),
 "B6b-mrtr-decline-taps": ("crates/cli/src/mcp.rs", "            match decision(reply) {\n                Decision::Declined => return Outcome::Done(declined()),\n                Decision::Dismissed => return Outcome::Done(dismissed(None)),\n                Decision::Once => globals.approval = Some(Approval::Once),\n                Decision::Session => globals.approval = Some(Approval::Session),\n            }\n            from = step;", "            match decision(reply) {\n                Decision::Declined => globals.approval = Some(Approval::Once),\n                Decision::Dismissed => return Outcome::Done(dismissed(None)),\n                Decision::Once => globals.approval = Some(Approval::Once),\n                Decision::Session => globals.approval = Some(Approval::Session),\n            }\n            from = step;"),
 "B9-dismissed-tap-taps-on-the-open-connection": ("crates/cli/src/mcp.rs", "            Decision::Dismissed => Outcome::Done(dismissed(Some(&asked))),\n            chosen => {", "            chosen => {"),
 "B3-held-batch-never-cleared": ("crates/cli/src/mcp.rs", "let held = self.held_batch.lock().await.take();", "let held = self.held_batch.lock().await.clone();"),
 "B4-any-retry-resumes-the-held-batch": ("crates/cli/src/mcp.rs", "held.filter(|(k, _)| *k == key)", "held"),
 "B5-session-approval-covers-every-category": ("crates/cli/src/daemon/guard.rs", "None if s.guard_allowed.contains(&category) => {", "None if !s.guard_allowed.is_empty() => {"),
 "B6-decline-taps-in-a-batch": ("crates/cli/src/mcp.rs", "                Decision::Declined => return Outcome::Done(declined_for(&asked)),\n                Decision::Dismissed => return Outcome::Done(dismissed(Some(&asked))),\n                Decision::Once => globals.approval = Some(Approval::Once),", "                Decision::Declined => globals.approval = Some(Approval::Once),\n                Decision::Dismissed => return Outcome::Done(dismissed(Some(&asked))),\n                Decision::Once => globals.approval = Some(Approval::Once),"),
 "B7-type-submit-is-not-guarded": ("crates/cli/src/daemon/actor.rs", "        let button = field\n            .zip(s.last.as_ref())\n            .and_then(|(f, now)| guard::submit_button(now, f, ask))\n            .cloned();", "        let button: Option<Element> = None;\n        let _ = (field, ask);"),
 "B8-opt-in-categories-ignored": ("crates/cli/src/daemon/guard.rs", "category.always() || ask.contains(&category)", "category.always()"),
}
if len(sys.argv) != 2 or sys.argv[1] not in BREAKS:
    sys.exit("usage: breaks.py <" + "|".join(BREAKS) + ">")
name = sys.argv[1]
f, old, new = BREAKS[name]
src = open(f).read()
assert src.count(old) == 1, (name, src.count(old))
open(f, "w").write(src.replace(old, new))
try:
    r = subprocess.run(["cargo","test","-p","phonedriver","--test","guard_property"], capture_output=True, text=True)
    out = r.stdout + r.stderr
finally:
    subprocess.run(["git","checkout","--",f])
open(f"{OUT}/break-{name}.txt","w").write(out)
print(name, "EXIT", r.returncode)
for l in out.splitlines():
    if l.startswith("test ") or l.startswith("Test failed") or "error[" in l or "error:" in l:
        print("   ", l[:300])
