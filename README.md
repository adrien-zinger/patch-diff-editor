# patch-diff-editor

Tool to mimic the behavior of `git add -p` or any git commands with `--patch`.
This editor can be used as `jujutsu` diff editor.

This program compare a `left` directory to a `right` directory and edit the
diffs, applying directly into `right` the selected hunks. So `right` is
modified.

## Usage

Modify the jj configuration.

```yaml
[ui]
diff-editor = "/<path>/patch-diff-editor"
```

The program works as follow.

```bash
patch-diff-editor <left> <right>
```

## Env var

For now, the configuration accept:

- `DPE_EDITOR`: program to launch to edit hunks (fallback `EDITOR`)
