use colored::Colorize;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct Hunk {
    diffs: Vec<Diff>,
    apply: bool,
}

#[derive(Debug, Clone)]
struct Diff {
    old_index: Option<usize>,
    new_index: Option<usize>,
    line: String,
    tag: ChangeTag,
}

impl Hunk {
    fn starting_indexes(&self) -> (Option<usize>, Option<usize>) {
        let start = &self.diffs[0];
        (start.old_index, start.new_index)
    }

    fn print(&self, file: &Path) {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let mut h = file
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|e| ps.find_syntax_by_extension(e))
            .map(|s| HighlightLines::new(s, &ts.themes["base16-ocean.dark"]));

        let (old_index, new_index) = self.starting_indexes();

        if let Some(file_str) = file.to_str() {
            println!("@@ {file_str}");
        }

        if let Some(old_index) = old_index {
            print!("@@ {old_index}");
        }
        if let Some(new_index) = new_index {
            println!(",{new_index}");
        } else {
            println!();
        }

        for diff in &self.diffs {
            match diff.tag {
                ChangeTag::Equal => {
                    if let Some(h) = &mut h {
                        for line in LinesWithEndings::from(&diff.line) {
                            let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
                            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
                            print!(" {escaped}");
                            print!("\x1b[0m");
                        }
                    } else {
                        print!(" {}", diff.line);
                    }
                }
                ChangeTag::Insert => print!("{}{}", "+".green(), diff.line.green()),
                ChangeTag::Delete => print!("{}{}", "-".red(), diff.line.red()),
            }
        }
    }
}

fn get_args() -> (String, String) {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <file1> <file2>", args[0]);
        std::process::exit(1);
    }
    (args[1].clone(), args[2].clone())
}

fn collect_files(root: &Path) -> HashMap<PathBuf, PathBuf> {
    let mut files = HashMap::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let abs_path = entry.path().to_path_buf();
        let rel_path = abs_path.strip_prefix(root).unwrap().to_path_buf();
        files.insert(rel_path, abs_path);
    }

    files
}

fn patch_dirs(left: &Path, right: &Path) -> io::Result<()> {
    let left_files = collect_files(left);
    let right_files = collect_files(right);

    // Files present in both → diff
    for rel_path in left_files.keys().filter(|p| right_files.contains_key(*p)) {
        let left_file = &left_files[rel_path];
        let right_file = &right_files[rel_path];

        let original = fs::read_to_string(left_file)?;
        let dest = fs::read_to_string(right_file)?;
        let dest = patch_file(right_file, &original, &dest)?;

        let mut right_file = File::create(right_file)?;
        right_file.write_all(dest.as_bytes())?;
    }

    // Files only in left → delete
    for rel_path in left_files.keys().filter(|p| !right_files.contains_key(*p)) {
        let left_file = &left_files[rel_path];
        let original = fs::read_to_string(left_file)?;

        print!("\n@@ delete {}\n@@:> ", left_file.to_str().unwrap());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            "y" | "" => {}
            "n" => {
                let mut right_file = File::create(right.join(rel_path))?;
                right_file.write_all(original.as_bytes())?;
            }
            "e" => {
                let dest = patch_file(left_file, &original, "")?;
                if !dest.is_empty() {
                    let mut right_file = File::create(right.join(rel_path))?;
                    right_file.write_all(dest.as_bytes())?;
                }
            }
            _ => println!("Unknown command"),
        }
    }

    // Files only in right → add
    for rel_path in right_files.keys().filter(|p| !left_files.contains_key(*p)) {
        let right_file = &right_files[rel_path];

        print!("\n@@ add {}\n@@:> ", right_file.to_str().unwrap());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "y" | "" => {}
            "n" => {
                fs::remove_file(right_file)?;
            }
            "e" => {
                let dest = fs::read_to_string(right_file)?;
                let dest = patch_file(right_file, "", &dest)?;
                let mut right_file = File::create(right_file)?;
                right_file.write_all(dest.as_bytes())?;
            }
            _ => println!("Unknown command"),
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let (left, right) = get_args();
    let left = Path::new(&left);
    let right = Path::new(&right);
    patch_dirs(left, right)?;
    Ok(())
}

fn patch_file(path: &Path, original: &str, dest: &str) -> io::Result<String> {
    let mut hunks = build_hunks(original, dest, 6);

    let mut i = 0;
    while i < hunks.len() {
        hunks[i].print(path);

        print!("\n@@:> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            "y" | "" => {
                hunks[i].apply = true;
                i += 1;
            }
            "n" => {
                hunks[i].apply = false;
                i += 1;
            }
            "e" => {
                hunks[i] = edit_hunk(original, &hunks[i]).unwrap().unwrap();
            }
            "s" => {
                let split = split_hunk(&hunks[i]);
                if split.len() > 1 {
                    hunks.remove(i);
                    for (n, h) in split.into_iter().enumerate() {
                        hunks.insert(i + n, h);
                    }
                } else {
                    println!("Hunk cannot be split further.");
                }
            }
            "q" => break,
            _ => println!("Unknown command"),
        }
    }

    Ok(apply(original, hunks))
}

fn edit_hunk(original: &str, hunk: &Hunk) -> io::Result<Option<Hunk>> {
    loop {
        let mut tmp = NamedTempFile::new()?;

        let (old_index, new_index) = hunk.starting_indexes();
        writeln!(tmp, "@@ {old_index:?},{new_index:?}").unwrap();
        for diff in &hunk.diffs {
            match diff.tag {
                ChangeTag::Equal => write!(tmp, " {}", diff.line)?,
                ChangeTag::Insert => write!(tmp, "+{}", diff.line)?,
                ChangeTag::Delete => write!(tmp, "-{}", diff.line)?,
            }
        }

        Command::new("vim").arg(tmp.path()).status()?;

        let mut edited = String::new();
        tmp.reopen()?.read_to_string(&mut edited)?;

        match check_patch(original, old_index.unwrap_or_default(), &edited) {
            Ok(new_hunk) => return Ok(Some(new_hunk)),
            Err(e) => {
                eprintln!("\nPatch does not apply:\n{e}\nPress Enter to re-edit…");
                let _ = io::stdin().read_line(&mut String::new());
            }
        }
    }
}

fn check_patch(original: &str, start: usize, new_patch: &str) -> Result<Hunk, String> {
    let diffs = new_patch
        .lines()
        .filter(|line| !line.starts_with("@@"))
        .map(|line| {
            let tag = match line.chars().next() {
                Some(' ') => ChangeTag::Equal,
                Some('+') => ChangeTag::Insert,
                Some('-') => ChangeTag::Delete,
                _ => return Err("Invalid diff format".into()),
            };

            Ok(Diff {
                line: format!("{}\n", &line[1..]),
                tag,
                old_index: Some(start),
                new_index: None,
            })
        })
        .collect::<Result<Vec<Diff>, String>>()?;

    apply_check(original, start, &diffs)?;
    Ok(Hunk {
        diffs,
        apply: false,
    })
}

fn apply_check(original: &str, old_index: usize, patch: &[Diff]) -> Result<(), String> {
    let original_lines: Vec<&str> = original.lines().collect();
    let mut index = old_index;

    for diff in patch {
        match diff.tag {
            ChangeTag::Equal => {
                if original_lines.get(index) != Some(&diff.line.as_str().trim_end()) {
                    println!("err, {:?} {}", original_lines.get(index), diff.line);
                    return Err(format!("Context mismatch at line {}", index + 1));
                }
                index += 1;
            }
            ChangeTag::Insert => {}
            ChangeTag::Delete => {
                index += 1;
            }
        }
    }

    println!("end of apply check");
    Ok(())
}

fn apply(original: &str, hunks: Vec<Hunk>) -> String {
    let mut ret = String::new();
    let original_lines: Vec<&str> = original.lines().collect();
    let mut index = 0;

    for hunk in hunks.into_iter().filter(|h| h.apply) {
        if let Some(old_index) = hunk.starting_indexes().0 {
            while index < old_index {
                ret.push_str(&format!("{}\n", original_lines[index]));
                index += 1;
            }
        }

        for diff in hunk.diffs {
            match diff.tag {
                ChangeTag::Equal => {
                    ret.push_str(&diff.line);
                    index += 1;
                }
                ChangeTag::Insert => {
                    ret.push_str(&diff.line);
                }
                ChangeTag::Delete => {
                    index += 1;
                }
            }
        }
    }

    while index < original_lines.len() {
        ret.push_str(&format!("{}\n", original_lines[index]));
        index += 1;
    }

    ret
}

fn build_hunks(a: &str, b: &str, context: usize) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(a, b);

    let mut hunks = vec![];
    let mut current = vec![];
    let mut in_diff = false;
    let mut trail = 0;

    for line in diff.iter_all_changes().map(|change| Diff {
        old_index: change.old_index(),
        new_index: change.new_index(),
        line: change.as_str().unwrap().into(),
        tag: change.tag(),
    }) {
        if line.old_index.is_none() && line.tag == ChangeTag::Equal {
            panic!("Invalid change detected {line:#?}");
        }

        current.push(line.clone());
        match line.tag {
            ChangeTag::Equal => {
                if in_diff {
                    trail += 1;
                    if trail >= context {
                        hunks.push(trim(std::mem::take(&mut current), context));
                        in_diff = false;
                        trail = 0;
                    }
                }
            }
            _ => {
                in_diff = true;
                trail = 0;
            }
        }
    }

    if !current.is_empty() && in_diff {
        hunks.push(trim(std::mem::take(&mut current), context));
    }

    hunks
}

fn trim(lines: Vec<Diff>, context: usize) -> Hunk {
    let first = lines
        .iter()
        .position(|change| change.tag != ChangeTag::Equal)
        .unwrap()
        .saturating_sub(context);

    let last = lines
        .iter()
        .rposition(|change| change.tag != ChangeTag::Equal)
        .unwrap();

    let last = if last + context > lines.len() {
        lines.len()
    } else {
        last + context
    };

    Hunk {
        diffs: lines[first..last].to_vec(),
        apply: false,
    }
}

fn split_hunk(hunk: &Hunk) -> Vec<Hunk> {
    let mut hunks = vec![];
    let mut in_diff = false;
    let mut current = vec![];

    for diff in &hunk.diffs {
        current.push(diff.clone());
        match diff.tag {
            ChangeTag::Equal => {
                if in_diff {
                    hunks.push(Hunk {
                        diffs: std::mem::take(&mut current),
                        apply: false,
                    });
                    in_diff = false;
                }
            }
            _ => {
                in_diff = true;
            }
        }
    }

    if in_diff {
        hunks.push(Hunk {
            diffs: current,
            apply: false,
        });
    }

    hunks
}
