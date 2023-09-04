use crate::Result;
use git2::build::CheckoutBuilder;
use git2::{Branch, BranchType, ObjectType, Repository};
use miette::{Context, IntoDiagnostic};
use std::fs::read_dir;
use std::path::Path;

pub fn checkout_all<P: AsRef<Path>>(sources_root: P, mut name: &str) -> Result<()> {
    // "main" and "master" are interchangeable
    if name == "main" {
        name = "master";
    }
    let apps_dir = sources_root.as_ref().join("apps");
    for app in read_dir(apps_dir).into_diagnostic()? {
        let app = app.into_diagnostic()?;
        if app.metadata().into_diagnostic()?.is_dir() && app.path().join(".git").is_dir() {
            let app_dir = app.path();
            let repo = Repository::init(&app_dir)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to open repository {}", app_dir.display()))?;
            if let Some(branch) = get_branch(&repo, name)? {
                if !branch.is_head() {
                    print!("{}", app.file_name().to_string_lossy());
                    if let Err(e) = checkout(&repo, branch) {
                        println!(": {:#} ❌", e);
                    } else {
                        println!(" ✓");
                    }
                }
            };
        }
    }
    Ok(())
}

fn get_branch<'repo>(repo: &'repo Repository, name: &str) -> Result<Option<Branch<'repo>>> {
    let branches = repo.branches(Some(BranchType::Local)).into_diagnostic()?;
    Ok(branches.flatten().find_map(|(branch, _)| {
        match (branch.name_bytes(), name) {
            (Ok(b"main"), "master") => Some(branch), // make "main" synonymous with "master"
            (branch_name, _) if branch_name == Ok(name.as_bytes()) => Some(branch),
            _ => None,
        }
    }))
}

fn checkout(repo: &Repository, branch: Branch) -> Result<()> {
    let commit = branch.get().peel(ObjectType::Commit).into_diagnostic()?;
    let name = branch
        .name()
        .into_diagnostic()?
        .expect("we already know the name if utf8");
    let mut checkout = CheckoutBuilder::default();
    checkout.update_index(true);

    repo.checkout_tree(&commit, Some(&mut checkout))
        .into_diagnostic()
        .wrap_err("Failed to checkout tree")?;
    repo.set_head(&format!("refs/heads/{name}"))
        .into_diagnostic()
        .wrap_err("Failed to set HEAD")
}
