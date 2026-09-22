"""KiCad PCB Editor (pcbnew) Action Plugin: adds an "OpenParts" toolbar
button that launches the openparts-kicad-plugin GUI app as an external
process, pre-filled with the currently open project's directory.

Install (Linux): copy or symlink this file into KiCad's user plugin
directory, e.g.:

    mkdir -p ~/.local/share/kicad/9.0/scripting/plugins
    ln -s ~/OpenParts/openparts-kicad-plugin/kicad-integration/openparts_launcher.py \
        ~/.local/share/kicad/9.0/scripting/plugins/openparts_launcher.py

(Adjust "9.0" to your installed KiCad version -- check Help > About KiCad,
or run `pcbnew` and look at Preferences > Configure Paths for the exact
scripting/plugins directory on your system.) Then restart KiCad, or use
Tools > External Plugins > Refresh Plugins.

This does not use KiCad's IPC API at all -- it uses the older, stable
pcbnew Python "Action Plugin" mechanism (the same one tools like
InteractiveHtmlBom use to add a toolbar button that shells out to an
external program), since the IPC API has no library-management support
to begin with. The actual OpenParts logic lives entirely in the Rust
binary; this script's only job is to find the current project directory
and launch that binary with it.
"""

import os
import subprocess

import pcbnew

# Path to the compiled openparts-kicad-plugin binary. Override by
# setting the OPENPARTS_KICAD_PLUGIN_BIN environment variable (e.g. in
# your shell profile, before starting KiCad), or just edit this default
# to match where you built it.
DEFAULT_BINARY_PATH = os.path.expanduser(
    "~/OpenParts/openparts-kicad-plugin/target/release/openparts-kicad-plugin"
)


class OpenPartsLauncher(pcbnew.ActionPlugin):
    def defaults(self):
        self.name = "OpenParts"
        self.category = "Read PCB"
        self.description = (
            "Search OpenParts and install a part's Symbol/Footprint/STEP "
            "into this project's local libraries"
        )
        self.show_toolbar_button = True
        self.icon_file_name = ""

    def Run(self):
        binary = os.environ.get("OPENPARTS_KICAD_PLUGIN_BIN", DEFAULT_BINARY_PATH)
        if not os.path.isfile(binary):
            pcbnew.Refresh()
            wx_message_box(
                "openparts-kicad-plugin binary not found at:\n{}\n\n"
                "Build it with `cargo build --release` in the "
                "openparts-kicad-plugin repo, or set the "
                "OPENPARTS_KICAD_PLUGIN_BIN environment variable.".format(binary)
            )
            return

        board = pcbnew.GetBoard()
        board_path = board.GetFileName() if board else ""
        args = [binary]
        if board_path:
            args += ["--project-dir", os.path.dirname(board_path)]

        subprocess.Popen(args)


def wx_message_box(text):
    try:
        import wx

        wx.MessageBox(text, "OpenParts", wx.OK | wx.ICON_WARNING)
    except Exception:
        print(text)


OpenPartsLauncher().register()
