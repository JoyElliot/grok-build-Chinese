async fn windows_activate_staged_exe(
    staged: &std::path::Path,
    dest: &std::path::Path,
    old: &std::path::Path,
) -> Result<()> {
    match tokio::fs::symlink_metadata(dest).await {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            windows_rename_no_replace(staged, dest)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            anyhow::bail!("replacement destination is not a regular file");
        }
        Ok(_) => {}
    }
    sweep_old_exe_backups(old).await;
    let mut aside = old.to_path_buf();
    let mut renamed = false;
    for attempt in 0..3 {
        match windows_rename_no_replace(dest, &aside) {
            Ok(()) => {
                renamed = true;
                break;
            }
            Err(error)
                if attempt < 2 && matches!(error.raw_os_error(), Some(5 | 32 | 80 | 183)) =>
            {
                aside = unique_temp_sibling(old, "old");
            }
            Err(error) => {
                return Err(error).context("moving installed executable to rollback backup");
            }
        }
    }
    if !renamed {
        anyhow::bail!("could not reserve an executable rollback backup");
    }
    match windows_rename_no_replace(staged, dest) {
        Ok(()) => {
            sweep_old_exe_backups(old).await;
            Ok(())
        }
        Err(error) => {
            // Never overwrite a file another process created during activation.
            // If that prevents rollback, retain the original and report its path.
            if let Err(rollback) = windows_rename_no_replace(&aside, dest) {
                anyhow::bail!(
                    "install failed: {error}; rollback failed: {rollback}; previous executable retained at {}",
                    aside.display()
                );
            }
            Err(error.into())
        }
    }
}
