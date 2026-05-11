---
name: setup
description: Check whether Contextro is installed and ready. Verifies the binary is in PATH and can respond to health checks.
disable-model-invocation: true
allowed-tools: Bash(contextro *) Bash(command *)
---

Check whether Contextro is installed and ready to use:

1. Run `command -v contextro` to verify the binary is in PATH.
2. If missing, tell the user to install with `npm install -g contextro`.
3. If present, run `contextro --version` to confirm it responds.
4. Report the status clearly.
