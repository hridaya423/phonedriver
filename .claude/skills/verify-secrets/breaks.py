import os, subprocess, sys

OUT = "target/verify-secrets"
BREAKS = {
    "B1-replies-are-not-masked": ("crates/cli/src/daemon/mod.rs", "        secrets::hide(&mut result, &secrets);\n", ""),
    "B2-a-secret-is-forgotten-after-its-own-call": ("crates/cli/src/daemon/secrets.rs", "if secret.chars().count() >= REMEMBERED_FROM {", "if secret.is_empty() {"),
    "B3-a-batch-step-is-not-read": ("crates/cli/src/daemon/secrets.rs", '"batch" => params["steps"]', '"batches" => params["steps"]'),
    "B4-a-shell-command-is-not-read": ("crates/cli/src/daemon/secrets.rs", '"shell" if params["secret"] == true', '"shell" if params["secret"] == 1'),
    "B5-the-trace-keeps-typed-text": ("crates/cli/src/daemon/trace.rs", 'action["text"] = length(if secret { "secret_len" } else { "text_len" }, &text);', "let _ = (secret, text);"),
    "B6-the-trace-keeps-a-secret-command": ("crates/cli/src/daemon/trace.rs", 'p["command"] = length("secret_len", &s);', "let _ = s;"),
    "B7-debug-prints-typed-text": ("crates/core/src/device.rs", 'write!(f, "{mode}({} chars)", text.chars().count())', 'write!(f, "{mode}({text})")'),
    "B8-debug-prints-a-shell-command": ("crates/core/src/device.rs", 'write!(f, "Shell({} chars)", self.command.chars().count())', 'write!(f, "Shell({})", self.command)'),
    "B9-a-failed-step-leaves-the-old-focus-trusted": ("crates/cli/src/daemon/actor.rs", ".filter(|_| !s.unsure)", ""),
    "B11-the-steps-file-of-a-recording-is-not-masked": ("crates/cli/src/daemon/actor.rs", "taping.step(began, &self.secrets.masked(&result.summary));", "taping.step(began, &result.summary);"),
    "B10-plain-text-goes-into-a-password-field": ("crates/cli/src/daemon/actor.rs", "                if !secret\n                    && !text.is_empty()\n                    && let Some((f, kind)) = field", "                if false\n                    && let Some((f, kind)) = field"),
}
if len(sys.argv) != 2 or sys.argv[1] not in BREAKS:
    sys.exit("usage: breaks.py <" + "|".join(BREAKS) + ">")
name = sys.argv[1]
path, old, new = BREAKS[name]
src = open(path).read()
assert src.count(old) == 1, (name, src.count(old))
os.makedirs(OUT, exist_ok=True)
open(path, "w").write(src.replace(old, new))
try:
    r = subprocess.run(["cargo", "test", "--locked", "-p", "phonedriver", "--test", "all", "secret_property::"], capture_output=True, text=True)
    out = r.stdout + r.stderr
finally:
    open(path, "w").write(src)
open(f"{OUT}/break-{name}.txt", "w").write(out)
print(name, "EXIT", r.returncode)
for line in out.splitlines():
    if line.startswith(("test ", "step ", "the ", "F")) or "error[" in line or "error:" in line:
        print("   ", line[:300])
