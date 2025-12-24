# patch-diff-editor

Tool to mimic the behavior of `git add -p` or any git commands with `--patch`.
This editor can be used as `jujutsu` diff editor.

```yaml
diff-editor = "/<path>/patch-diff-editor"
```

## Usage

```bash
patch-diff-editor <left> <right>
```

## Env var

For now, the configuration accept:

- `DPE_EDITOR`: program to launch to edit hunks (fallback `EDITOR`)
