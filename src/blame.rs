use crate::repository::Repository;
use crate::signature::Signature;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::ops::Deref;
use std::path::Path;

const MAX_SCAN_LINES: u32 = 10000;

#[napi(object)]
/// Represents a hunk of a blame operation, which is a range of lines
/// and information about who last modified them.
pub struct BlameHunk {
  /// The oid of the commit where this line was last changed.
  pub commit_id: String,
  /// The 1-based line number in the final file where this hunk starts.
  pub final_start_line_number: u32,
  /// The number of lines in this hunk.
  pub lines_in_hunk: u32,
  /// The signature of the commit where this line was last changed.
  pub signature: Option<Signature>,
  /// The path to the file where this line was originally written.
  pub path: Option<String>,
  /// The 1-based line number in the original file where this hunk starts.
  pub orig_start_line_number: u32,
  /// True if the hunk has been determined to be a boundary commit (the commit
  /// when the file was first introduced to the repository).
  pub is_boundary: bool,
}

#[napi(object)]
#[derive(Default)]
/// Options for controlling blame behavior
pub struct BlameOptions {
  /// The oid of the newest commit to consider. The blame algorithm will stop
  /// when this commit is reached.
  pub newest_commit: Option<String>,
  /// The oid of the oldest commit to consider. The blame algorithm will
  /// stop when this commit is reached.
  pub oldest_commit: Option<String>,
  /// The path to the file being worked on. Path has to be relative to the
  /// repo root.
  pub path: Option<String>,
  /// Track lines that have moved within a file. This is the git-blame -M
  /// option.
  pub track_lines_movement: Option<bool>,
}

impl BlameOptions {
  /// Set the path to the file
  pub fn with_path(mut self, path: &str) -> Self {
    self.path = Some(path.to_string());
    self
  }

  /// Set the newest commit to consider
  pub fn with_newest_commit(mut self, commit: &str) -> Self {
    self.newest_commit = Some(commit.to_string());
    self
  }

  /// Set the oldest commit to consider
  pub fn with_oldest_commit(mut self, commit: &str) -> Self {
    self.oldest_commit = Some(commit.to_string());
    self
  }

  /// Set whether to track line movements
  pub fn with_track_lines_movement(mut self, track: bool) -> Self {
    self.track_lines_movement = Some(track);
    self
  }
}

impl From<&BlameOptions> for git2::BlameOptions {
  fn from(options: &BlameOptions) -> Self {
    let mut git_opts = git2::BlameOptions::new();

    if let Some(ref newest_commit) = options.newest_commit {
      if let Ok(oid) = git2::Oid::from_str(newest_commit) {
        git_opts.newest_commit(oid);
      }
    }

    if let Some(ref oldest_commit) = options.oldest_commit {
      if let Ok(oid) = git2::Oid::from_str(oldest_commit) {
        git_opts.oldest_commit(oid);
      }
    }

    if let Some(track_lines_movement) = options.track_lines_movement {
      git_opts.track_copies_same_file(track_lines_movement);
    }

    git_opts
  }
}

#[napi]
pub struct Blame {
  pub(crate) inner: BlameInner,
}

pub(crate) enum BlameInner {
  Repo(SharedReference<Repository, git2::Blame<'static>>),
}

impl Deref for BlameInner {
  type Target = git2::Blame<'static>;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Repo(repo) => repo.deref(),
    }
  }
}

#[napi]
impl Blame {
  #[napi]
  /// Gets the number of hunks in the blame result
  ///
  /// @category Blame/Methods
  /// @signature
  /// ```ts
  /// class Blame {
  ///   getHunkCount(): number;
  /// }
  /// ```
  ///
  /// @returns The number of hunks in the blame result
  pub fn get_hunk_count(&self) -> u32 {
    self.inner.len() as u32
  }

  #[napi]
  /// Gets blame information for the specified line
  ///
  /// @category Blame/Methods
  /// @signature
  /// ```ts
  /// class Blame {
  ///   getHunkByLine(line: number): BlameHunk;
  /// }
  /// ```
  ///
  /// @param {number} line - The line number to get blame information for (1-based)
  /// @returns Blame information for the specified line
  /// @throws If no hunk is found for the specified line
  pub fn get_hunk_by_line(&self, line: u32) -> Result<BlameHunk> {
    let hunk = self
      .inner
      .get_line(line as usize)
      .ok_or_else(|| Error::from_reason(format!("No blame hunk found for line {}", line)))?;

    let signature = Signature::try_from(hunk.final_signature()).ok();
    let path = hunk.path().map(|p| p.to_string_lossy().to_string());

    Ok(BlameHunk {
      commit_id: hunk.final_commit_id().to_string(),
      final_start_line_number: hunk.final_start_line() as u32,
      lines_in_hunk: hunk.lines_in_hunk() as u32,
      signature,
      path,
      orig_start_line_number: hunk.orig_start_line() as u32,
      is_boundary: hunk.is_boundary(),
    })
  }

  #[napi]
  /// Gets an array of blame hunks for all lines
  ///
  /// @category Blame/Methods
  /// @signature
  /// ```ts
  /// class Blame {
  ///   getHunks(): BlameHunk[];
  /// }
  /// ```
  ///
  /// @returns Array of blame hunks
  pub fn get_hunks(&self) -> Result<Vec<BlameHunk>> {
    let hunk_count = self.get_hunk_count() as usize;

    if hunk_count == 0 {
      return Ok(Vec::new());
    }

    let mut hunks = Vec::with_capacity(hunk_count);
    let mut seen_hunks = HashSet::new();
    let mut line = 1;

    while hunks.len() < hunk_count && line < MAX_SCAN_LINES {
      if let Ok(hunk) = self.get_hunk_by_line(line) {
        let hunk_key = (hunk.final_start_line_number, hunk.lines_in_hunk);

        if seen_hunks.insert(hunk_key) {
          line += hunk.lines_in_hunk;
          hunks.push(hunk);
          continue;
        }
      }

      line += 1;
    }

    Ok(hunks)
  }
}

#[napi]
impl Repository {
  fn get_blame_with_options(
    &self,
    path: String,
    min_line: Option<u32>,
    max_line: Option<u32>,
    options: Option<BlameOptions>,
    this: Reference<Repository>,
    env: Env,
  ) -> Result<Blame> {
    let file_path = Path::new(&path);

    let blame = this.share_with(env, |repo| {
      let mut git_options = match &options {
        Some(options) => git2::BlameOptions::from(options),
        None => git2::BlameOptions::new(),
      };

      if let Some(min) = min_line {
        git_options.min_line(min as usize);
      }

      if let Some(max) = max_line {
        git_options.max_line(max as usize);
      }

      repo
        .inner
        .blame_file(file_path, Some(&mut git_options))
        .map_err(|e| Error::from_reason(e.to_string()))
    })?;

    Ok(Blame {
      inner: BlameInner::Repo(blame),
    })
  }

  #[napi]
  /// Get blame hunks for the entire file
  ///
  /// @category Repository/Methods
  /// @signature
  /// ```ts
  /// class Repository {
  ///   getBlame(path: string, options?: BlameOptions | null | undefined): BlameHunk[];
  /// }
  /// ```
  ///
  /// @example
  /// ```ts
  /// // Get blame hunks for the entire file
  /// const hunks = repo.getBlame('path/to/file.js');
  /// ```
  ///
  /// @param {string} path - Path to the file to blame
  /// @param {BlameOptions} [options] - Options to control blame behavior
  /// @returns Array of blame hunks for the file
  pub fn get_blame(
    &self,
    path: String,
    options: Option<BlameOptions>,
    this: Reference<Repository>,
    env: Env,
  ) -> Result<Vec<BlameHunk>> {
    let blame = self.get_blame_with_options(path, None, None, options, this, env)?;

    blame.get_hunks()
  }

  #[napi]
  /// Get blame hunk for a specific line in a file
  ///
  /// @category Repository/Methods
  /// @signature
  /// ```ts
  /// class Repository {
  ///   getBlameLine(path: string, line: number, options?: BlameOptions | null | undefined): BlameHunk;
  /// }
  /// ```
  ///
  /// @example
  /// ```ts
  /// // Get blame hunk for line 10
  /// const hunk = repo.getBlameLine('path/to/file.js', 10);
  /// ```
  ///
  /// @param {string} path - Path to the file to blame
  /// @param {number} line - The line number to get blame information for (1-based)
  /// @param {BlameOptions} [options] - Options to control blame behavior
  /// @returns Blame hunk for the specified line
  pub fn get_blame_line(
    &self,
    path: String,
    line: u32,
    options: Option<BlameOptions>,
    this: Reference<Repository>,
    env: Env,
  ) -> Result<BlameHunk> {
    let blame = self.get_blame_with_options(path, Some(line), Some(line), options, this, env)?;

    blame.get_hunk_by_line(line)
  }

  #[napi]
  /// Get blame hunks for a range of lines in a file
  ///
  /// @category Repository/Methods
  /// @signature
  /// ```ts
  /// class Repository {
  ///   getBlameRange(path: string, startLine: number, endLine: number, options?: BlameOptions | null | undefined): BlameHunk[];
  /// }
  /// ```
  ///
  /// @example
  /// ```ts
  /// // Get blame hunks for lines 5-15
  /// const hunks = repo.getBlameRange('path/to/file.js', 5, 15);
  /// ```
  ///
  /// @param {string} path - Path to the file to blame
  /// @param {number} startLine - The starting line number (1-based)
  /// @param {number} endLine - The ending line number (1-based)
  /// @param {BlameOptions} [options] - Options to control blame behavior
  /// @returns Array of blame hunks for the specified range
  pub fn get_blame_range(
    &self,
    path: String,
    start_line: u32,
    end_line: u32,
    options: Option<BlameOptions>,
    this: Reference<Repository>,
    env: Env,
  ) -> Result<Vec<BlameHunk>> {
    if start_line > end_line {
      return Err(Error::from_reason(format!(
        "Invalid range: start line ({}) must be less than or equal to end line ({})",
        start_line, end_line
      )));
    }

    let blame = self.get_blame_with_options(path, Some(start_line), Some(end_line), options, this, env)?;

    let hunks = blame.get_hunks()?;

    let filtered_hunks: Vec<BlameHunk> = hunks
      .into_iter()
      .filter(|hunk| {
        let hunk_start = hunk.final_start_line_number;
        let hunk_end = hunk_start + hunk.lines_in_hunk - 1;

        (hunk_start <= end_line) && (hunk_end >= start_line)
      })
      .collect();

    Ok(filtered_hunks)
  }
}
