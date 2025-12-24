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

## Cli

User inputs

```bash
@@:> h
y - apply this hunk
n - skip this hunk
e - edit the current hunk manually
s - split the current hunk into smaller hunks
h - show this help
q - quit; do not process any more hunks
a - apply all nexts hunks for this file
d - skip all nexts hunks for this file
j - go to the previous hunk

Press Enter is the same as 'y'.
```
