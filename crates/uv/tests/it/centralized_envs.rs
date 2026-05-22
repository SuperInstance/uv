use anyhow::Result;
use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use indoc::indoc;

use uv_fs::Simplified;
use uv_static::EnvVars;

use uv_test::uv_snapshot;

/// `cache clean` should remove centralized environments.
#[test]
fn clean_removes_centralized_env() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]).with_filtered_counts();

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = ["iniconfig"]
        "#,
    )?;

    // Create a centralized environment.
    context
        .sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .assert()
        .success();

    // `.venv` should be a symlink (Unix) or junction (Windows) pointing into the cache.
    let venv_path = context.temp_dir.child(".venv").path().to_path_buf();
    let link_target = fs_err::read_link(&venv_path)?;
    assert!(link_target.exists(), "Centralized environment should exist");

    // Clean the cache.
    uv_snapshot!(context.filters(), context.clean(), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Clearing cache at: [CACHE_DIR]/
    Removed [N] files ([SIZE])
    ");

    assert!(
        !link_target.exists(),
        "Centralized environment should have been removed by cache clean"
    );

    Ok(())
}

/// `uv run` with centralized environments should work end-to-end.
#[test]
fn run_centralized_env() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(indoc! { r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#
    })?;

    uv_snapshot!(context.filters(), context.run()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("python")
        .arg("-c")
        .arg("print('hello')"), @"
    success: true
    exit_code: 0
    ----- stdout -----
    hello

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // `.venv` should be a symlink/junction pointing into the cache.
    let link_target = fs_err::read_link(context.temp_dir.child(".venv").path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    // A second run should reuse the environment without creating it again.
    uv_snapshot!(context.filters(), context.run()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("python")
        .arg("-c")
        .arg("print('hello again')"), @"
    success: true
    exit_code: 0
    ----- stdout -----
    hello again

    ----- stderr -----
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    Ok(())
}

/// Test that `uv sync --preview-features centralized-envs` creates the environment
/// in the cache and symlinks `.venv` to it.
#[test]
fn sync_centralized_env() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = ["iniconfig"]
        "#,
    )?;

    // Running `uv sync` with centralized-envs should create the environment in the cache.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 2 packages in [TIME]
    Prepared 1 package in [TIME]
    Installed 1 package in [TIME]
     + iniconfig==2.0.0
    ");

    // `.venv` should be a symlink/junction pointing into the cache.
    let link_target = fs_err::read_link(context.temp_dir.child(".venv").path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    // A re-sync should reuse the existing environment.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Resolved 2 packages in [TIME]
    Checked 1 package in [TIME]
    ");

    // The link should still be there.
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_ok());

    Ok(())
}

/// On Windows, centralized environments on SMB cannot be linked from a local workspace.
///
/// Requires `UV_INTERNAL__TEST_SMB_FS`.
#[test]
#[cfg(windows)]
fn sync_centralized_env_smb_cache_writes_path_file() -> Result<()> {
    let Some(context) = uv_test::test_context_with_versions!(&["3.12"]).with_cache_on_smb_fs()?
    else {
        return Ok(());
    };

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @r"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    warning: Failed to create symlink, wrote a path file instead: The filename, directory name, or volume label syntax is incorrect. (os error 123)
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    let venv = context.temp_dir.child(".venv");
    assert!(venv.is_file());
    assert!(fs_err::read_link(venv.path()).is_err());

    let link_target = fs_err::read_to_string(venv.path())?;
    let link_target = std::path::Path::new(&link_target);
    assert!(link_target.is_dir());
    assert!(link_target.join("pyvenv.cfg").is_file());
    let link_target = link_target.strip_prefix(context.cache_dir.path())?;

    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"environments-v2/project-py3.12-[HASH]"
        );
    });

    uv_snapshot!(context.filters(), context.run()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("python")
        .arg("-c")
        .arg("print('hello')"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    hello

    ----- stderr -----
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    Ok(())
}

/// Test that `uv sync --preview-features centralized-envs` creates and uses a centralised
/// environment despite an existing real `.venv` directory. And that it clobbers it to create the
/// link.
#[test]
fn sync_centralized_env_existing_local_venv() -> Result<()> {
    let context = uv_test::test_context!("3.12");

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = ["iniconfig"]
        "#,
    )?;

    // First, create a local .venv without the centralized flag.
    uv_snapshot!(context.filters(), context.sync(), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Resolved 2 packages in [TIME]
    Prepared 1 package in [TIME]
    Installed 1 package in [TIME]
     + iniconfig==2.0.0
    ");

    // `.venv` should not be a normal directory and not secretly a symlink/junction.
    assert!(context.temp_dir.child(".venv").is_dir());
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_err());

    // Now sync with the centralized flag. It should create a new centralised environment.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 2 packages in [TIME]
    Installed 1 package in [TIME]
     + iniconfig==2.0.0
    ");

    // `.venv` should now be a symlink/junction.
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_ok());

    Ok(())
}

/// When `UV_PROJECT_ENVIRONMENT` is set, centralized mode should be bypassed.
#[test]
fn sync_centralized_env_project_environment_override() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = ["iniconfig"]
        "#,
    )?;

    let custom_env = context.temp_dir.join("custom-env");

    // Sync with both centralized-envs and UV_PROJECT_ENVIRONMENT.
    // UV_PROJECT_ENVIRONMENT should win.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .env(EnvVars::UV_PROJECT_ENVIRONMENT, &custom_env), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment at: custom-env
    Resolved 2 packages in [TIME]
    Prepared 1 package in [TIME]
    Installed 1 package in [TIME]
     + iniconfig==2.0.0
    ");

    // The environment should be at the custom path, not centralized.
    assert!(custom_env.is_dir());
    // `.venv` should not exist at all.
    assert!(!context.temp_dir.child(".venv").path().exists());
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_err());

    Ok(())
}

/// Two projects with the same name but different paths should get different
/// centralized environments (no collision).
#[test]
fn sync_centralized_env_no_collision() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let project_a = context.temp_dir.child("project-a");
    project_a.create_dir_all()?;
    project_a.child("pyproject.toml").write_str(
        r#"
        [project]
        name = "my-project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    let project_b = context.temp_dir.child("project-b");
    project_b.create_dir_all()?;
    project_b.child("pyproject.toml").write_str(
        r#"
        [project]
        name = "my-project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    // Sync project A.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("--project")
        .arg(project_a.path()), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `my-project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // Sync project B.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("--project")
        .arg(project_b.path()), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `my-project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // Both should have .venv symlinks pointing to different centralized envs.
    let link_a = fs_err::read_link(project_a.child(".venv").path())?;
    let link_b = fs_err::read_link(project_b.child(".venv").path())?;
    assert_ne!(
        link_a, link_b,
        "Projects with same name but different paths should have different centralized envs"
    );

    Ok(())
}

/// A virtual workspace root (no `[project]` table) should still work with centralized envs.
#[test]
fn sync_centralized_env_virtual_workspace() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    // Create a virtual workspace root with a member.
    context.temp_dir.child("pyproject.toml").write_str(
        r#"
        [tool.uv.workspace]
        members = ["member"]
        "#,
    )?;

    let member = context.temp_dir.child("member");
    member.create_dir_all()?;
    member.child("pyproject.toml").write_str(
        r#"
        [project]
        name = "member"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = ["iniconfig"]
        "#,
    )?;

    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `temp-py3.12-[HASH]` in the centralized store
    Resolved 2 packages in [TIME]
    Prepared 1 package in [TIME]
    Installed 1 package in [TIME]
     + iniconfig==2.0.0
    ");

    // `.venv` link should be at the workspace root, not the member.
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_ok());
    assert!(!member.child(".venv").exists());

    Ok(())
}

/// Switching Python versions with centralized envs should create a new environment
/// and update the `.venv` symlink.
#[test]
fn sync_centralized_env_switch_python() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.11", "3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.11"
        dependencies = []
        "#,
    )?;

    // Sync with 3.12.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("-p")
        .arg("3.12"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    let link_312 = fs_err::read_link(context.temp_dir.child(".venv").path())?;

    // Sync with 3.11.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("-p")
        .arg("3.11"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.11.[X] interpreter at: [PYTHON-3.11]
    Creating virtual environment `project-py3.11-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    let link_311 = fs_err::read_link(context.temp_dir.child(".venv").path())?;

    // The two environments should be different (different Python version = different hash).
    assert_ne!(
        link_312, link_311,
        "Different Python versions should produce different centralized environments"
    );

    // Switching back to 3.12 should reuse the original environment.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("-p")
        .arg("3.12"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    let link_312_again = fs_err::read_link(context.temp_dir.child(".venv").path())?;
    assert_eq!(
        link_312, link_312_again,
        "Should reuse the original 3.12 environment"
    );

    Ok(())
}

/// When `--active` is set and `VIRTUAL_ENV` points to a different environment,
/// centralized mode should be skipped and the active environment should be used.
#[test]
fn sync_centralized_env_active_overrides() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    // Create a separate virtual environment to use as the active env.
    let active_venv = context.temp_dir.child("my-active-env");
    uv_snapshot!(context.filters(), context.venv()
        .arg(active_venv.path())
        .arg("-p")
        .arg("3.12"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment at: my-active-env
    Activate with: source my-active-env/[BIN]/activate
    ");

    // Sync with --active and VIRTUAL_ENV pointing to the separate env.
    // Centralized mode should be skipped; the active env should be used directly.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--active")
        .arg("--preview-features")
        .arg("centralized-envs")
        .env("VIRTUAL_ENV", active_venv.path()), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    assert!(
        fs_err::read_link(context.temp_dir.child(".venv").path()).is_err(),
        "No .venv symlink should be created when --active overrides centralized mode"
    );

    Ok(())
}

/// When `--active` is set but `VIRTUAL_ENV` is not set, centralized mode should
/// still apply normally.
#[test]
fn sync_centralized_env_active_without_virtual_env() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    // Sync with --active but no VIRTUAL_ENV set.
    // Should fall through to centralized mode.
    uv_snapshot!(context.filters(), context.sync()
        .arg("--active")
        .arg("--preview-features")
        .arg("centralized-envs")
        .env_remove("VIRTUAL_ENV"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    let link_target = fs_err::read_link(context.temp_dir.child(".venv").path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    Ok(())
}

/// Test centralized mode with various pre-existing `.venv` states:
/// symlink, plain file, empty directory, and non-empty non-venv directory.
#[test]
fn sync_centralized_env_existing_venv_states() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        dependencies = []
        "#,
    )?;

    let venv = context.temp_dir.child(".venv");

    // Case 1: .venv is an existing symlink/junction pointing somewhere else
    let other_dir = context.temp_dir.child("other-dir");
    other_dir.create_dir_all()?;
    uv_fs::create_symlink(other_dir.path(), venv.path())?;

    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // Centralized mode should replace the symlink.
    let link_target = fs_err::read_link(venv.path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    assert!(
        other_dir.is_dir(),
        "The overridden symlink should not have affected its target"
    );

    fs_err::remove_dir_all(venv.path())?;

    // Case 2: .venv is an existing plain file
    fs_err::write(venv.path(), "foo bar baz")?;
    assert!(venv.path().is_file());

    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // Centralized mode should replace the plain file.
    let link_target = fs_err::read_link(venv.path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    fs_err::remove_dir_all(venv.path())?;

    // Case 3: .venv is an empty directory.
    fs_err::create_dir(venv.path())?;
    assert!(venv.path().is_dir());
    assert!(fs_err::read_dir(venv.path())?.next().is_none());

    // On "Unix" there is no legitimate reason for this to happen, so it leads to a failure to
    // create the `.venv` link which is only a warning.
    #[cfg(unix)]
    {
        uv_snapshot!(context.filters(), context.sync()
            .arg("--preview-features")
            .arg("centralized-envs"), @"
        success: true
        exit_code: 0
        ----- stdout -----

        ----- stderr -----
        warning: Failed to create symlink or path file: failed to rename file from [TEMP_DIR]/[TMP]/: Is a directory (os error 21)
        Resolved 1 package in [TIME]
        Checked in [TIME]
        ");
        fs_err::remove_dir(venv.path())?;
    }

    // On Windows, empty directories can be left behind when copying junctions between drives. So
    // this should succeed.
    #[cfg(windows)]
    {
        uv_snapshot!(context.filters(), context.sync()
            .arg("--preview-features")
            .arg("centralized-envs"), @"
        success: true
        exit_code: 0
        ----- stdout -----

        ----- stderr -----
        Resolved 1 package in [TIME]
        Checked in [TIME]
        ");
        fs_err::remove_dir_all(venv.path())?;
    }

    // Case 4: .venv is a non-empty directory without pyvenv.cfg
    fs_err::create_dir(venv.path())?;
    fs_err::write(venv.path().join("some-file.txt"), "not a venv")?;

    // Non-empty directories can't be replaced - a warning is emitted.

    // On "Unix" the warning concerns the symlink creation, because we don't attempt to remove
    // pre-existing directories.
    #[cfg(unix)]
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    warning: Failed to create symlink or path file: failed to rename file from [TEMP_DIR]/[TMP]/: Is a directory (os error 21)
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    // On Windows the warning concerns directory removal as part of the junction creation.
    #[cfg(windows)]
    uv_snapshot!(context.filters(), context.sync()
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    warning: Failed to create symlink or path file: failed to remove directory `[VENV]/`: The directory is not empty. (os error 145)
    Resolved 1 package in [TIME]
    Checked in [TIME]
    ");

    Ok(())
}

/// Test that `uv venv --preview-features centralized-envs` creates the environment
/// in the cache and symlinks `.venv` to it.
#[test]
fn create_venv_centralized() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        "#,
    )?;

    // Create a centralized environment (no explicit path arg).
    uv_snapshot!(context.filters(), context.venv()
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Activate with: source .venv/[BIN]/activate
    "
    );

    // `.venv` should be a symlink/junction pointing into the cache.
    let link_target = fs_err::read_link(context.temp_dir.child(".venv").path())?;
    insta::with_settings!({ filters => context.filters() }, {
        insta::assert_snapshot!(
            link_target.portable_display().to_string(),
            @"[CACHE_DIR]/environments-v2/project-py3.12-[HASH]"
        );
    });

    Ok(())
}

/// Test that `uv venv --preview-features centralized-envs` does not complain when a centralized
/// environment already exists.
#[test]
fn create_venv_centralized_already_exists() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        "#,
    )?;

    // Create a centralized environment.
    uv_snapshot!(context.filters(), context.venv()
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Activate with: source .venv/[BIN]/activate
    "
    );

    // Running again should fail with "already exists".
    uv_snapshot!(context.filters(), context.venv()
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Activate with: source .venv/[BIN]/activate
    "
    );

    // Running with `--no-clear` should make no difference.
    uv_snapshot!(context.filters(), context.venv()
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs")
        .arg("--no-clear"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment `project-py3.12-[HASH]` in the centralized store
    Activate with: source .venv/[BIN]/activate
    "
    );

    Ok(())
}

/// Test that `uv venv --preview-features centralized-envs` with an explicit path
/// ignores centralized mode.
#[test]
fn create_venv_centralized_explicit_path() -> Result<()> {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    let pyproject_toml = context.temp_dir.child("pyproject.toml");
    pyproject_toml.write_str(
        r#"
        [project]
        name = "project"
        version = "0.1.0"
        requires-python = ">=3.12"
        "#,
    )?;

    let custom_path = context.temp_dir.child("my-env");

    // Create a venv with an explicit path. Should create locally, not centralized.
    uv_snapshot!(context.filters(), context.venv()
        .arg(custom_path.as_os_str())
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment at: my-env
    Activate with: source my-env/[BIN]/activate
    "
    );

    // `my-env` should be a real directory, not a symlink/junction.
    assert!(custom_path.is_dir());
    assert!(fs_err::read_link(&custom_path).is_err());

    // `.venv` should not exist.
    assert!(!context.temp_dir.child(".venv").exists());

    Ok(())
}

/// `uv venv --preview-features centralized-envs` outside a project should
/// create a local `.venv` as normal (centralized mode requires a project).
#[test]
fn create_venv_centralized_no_project() {
    let context = uv_test::test_context_with_versions!(&["3.12"]);

    uv_snapshot!(context.filters(), context.venv()
        .arg("--python")
        .arg("3.12")
        .arg("--preview-features")
        .arg("centralized-envs"), @"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    Using CPython 3.12.[X] interpreter at: [PYTHON-3.12]
    Creating virtual environment at: .venv
    Activate with: source .venv/[BIN]/activate
    "
    );

    // `.venv` should be a real directory, not a symlink.
    assert!(context.temp_dir.child(".venv").is_dir());
    assert!(fs_err::read_link(context.temp_dir.child(".venv").path()).is_err());
}
