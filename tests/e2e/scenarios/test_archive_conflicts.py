# SPDX-License-Identifier: MIT

import tarfile
import zipfile

import pytest

from harness.modes import ALL_MODES


def request_archive_collision(strata, format="ZIP"):
    strata.open_context_menu("todo.txt")
    strata.choose_menu_item("Compress…")
    dialog = strata.wait_for_dialog()
    field = strata.editable_field()
    strata.keyboard.press("ctrl+a")
    strata.keyboard.type_text("archive")
    strata.wait(lambda: field.text == "archive", "archive name input")
    strata.pointer.click(dialog.find(role="toggle button", name=format))
    strata.pointer.click(strata.dialog_button("Compress"))
    strata.wait(
        lambda: (dialog := strata.dialog()) is not None and dialog.name == "File already exists",
        "archive collision prompt",
    )


@pytest.mark.parametrize("mode", ALL_MODES)
@pytest.mark.parametrize("format,extension", [("ZIP", "zip"), ("TAR.GZ", "tar.gz")])
@pytest.mark.parametrize("activation", ["pointer", "Return"])
def test_keep_both_preserves_archives_and_selects_each_numbered_output(strata, mode, format, extension, activation):
    fixture = strata.fixture
    original = fixture.path(f"archive.{extension}")
    previous = fixture.path(f"archive (1).{extension}")
    original.write_bytes(b"original archive")
    previous.write_bytes(b"previous archive")
    for suffix in [2, 3]:
        request_archive_collision(strata, format)
        if activation == "pointer":
            strata.pointer.click(strata.dialog_button("Keep Both"))
        else:
            strata.wait(lambda: strata.dialog_button("Replace").has_state("focused"), "initial Replace focus")
            strata.keyboard.press("shift+Tab")
            strata.wait(lambda: strata.dialog_button("Keep Both").has_state("focused"), "Keep Both focus")
            strata.keyboard.press("Return")
        name = f"archive ({suffix}).{extension}"
        path = fixture.path(name)
        strata.wait(path.exists, "numbered archive publication")
        strata.wait(lambda: strata.dialog() is None, "archive progress dismissal")
        strata.wait_for_selection([name])
        strata.wait(lambda: strata.on_screen(strata.entry(name)), "numbered archive reveal")
        if format == "ZIP":
            with zipfile.ZipFile(path) as archive:
                assert archive.read("todo.txt") == fixture.path("todo.txt").read_bytes()
        else:
            with tarfile.open(path, "r:gz") as archive:
                assert archive.extractfile("todo.txt").read() == fixture.path("todo.txt").read_bytes()
        assert original.read_bytes() == b"original archive"
        assert previous.read_bytes() == b"previous archive"


@pytest.mark.parametrize("choice", ["Cancel", "Escape", "Replace"])
def test_archive_conflict_keyboard_choices_preserve_cancel_and_replace_behavior(strata, choice):
    original = strata.fixture.path("archive.zip")
    original.write_bytes(b"original archive")
    request_archive_collision(strata)
    strata.wait(lambda: strata.dialog_button("Replace").has_state("focused"), "initial Replace focus")
    if choice == "Escape":
        strata.keyboard.press("Escape")
    else:
        if choice == "Cancel":
            strata.keyboard.press("shift+Tab")
            strata.keyboard.press("shift+Tab")
            strata.wait(lambda: strata.dialog_button("Cancel").has_state("focused"), "Cancel focus")
        strata.keyboard.press("Return")
    strata.wait(lambda: strata.dialog() is None, "conflict dismissal")
    assert not strata.fixture.path("archive (1).zip").exists()
    if choice == "Replace":
        with zipfile.ZipFile(original) as archive:
            assert archive.read("todo.txt") == strata.fixture.path("todo.txt").read_bytes()
    else:
        assert original.read_bytes() == b"original archive"
