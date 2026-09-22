"""KiCad PCB Editor (pcbnew) Action Plugin: adds an "OpenParts" toolbar
button that launches the openparts-kicad-plugin GUI app as an external
process, pre-filled with the currently open project's directory.

Normally you won't place this file by hand -- see install.sh (Linux) or
install.ps1 (Windows) in the repo root, which download a prebuilt
release binary, copy this script into KiCad's plugin directory, and
write the config file `resolve_binary_path()` below reads to find the
binary. Manual install (Linux example):

    mkdir -p ~/.local/share/kicad/9.0/scripting/plugins
    ln -s ~/OpenParts/openparts-kicad-plugin/kicad-integration/openparts_launcher.py \
        ~/.local/share/kicad/9.0/scripting/plugins/openparts_launcher.py

(Adjust "9.0" to your installed KiCad version -- check Help > About KiCad.
The authoritative way to find this directory on any OS/version is KiCad's
own Tools > External Plugins > Open Plugin Directory.) Then restart
KiCad, or use Tools > External Plugins > Refresh Plugins.

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

# Fallback path to the compiled openparts-kicad-plugin binary, used only
# if neither the OPENPARTS_KICAD_PLUGIN_BIN environment variable nor the
# installer's config file (see resolve_binary_path) resolves one. Edit
# this if you built from source by hand instead of using an installer.
DEFAULT_BINARY_PATH = os.path.expanduser(
    "~/OpenParts/openparts-kicad-plugin/target/release/openparts-kicad-plugin"
)


def resolve_binary_path():
    """Finds the openparts-kicad-plugin binary, checked in order:

    1. The OPENPARTS_KICAD_PLUGIN_BIN environment variable -- convenient
       for manual setups, but GUI application launchers (desktop icons,
       taskbars) often don't inherit shell profile environment
       variables, so this alone isn't reliable for an installer script
       to depend on.
    2. A config file install.sh/install.ps1 write, independent of any
       shell session: %APPDATA%\\openparts-kicad-plugin\\binary_path.txt
       on Windows, ~/.config/openparts-kicad-plugin/binary_path.txt
       elsewhere.
    3. DEFAULT_BINARY_PATH, for manual `cargo build` setups.
    """
    env_override = os.environ.get("OPENPARTS_KICAD_PLUGIN_BIN")
    if env_override:
        return env_override

    appdata = os.environ.get("APPDATA")
    if appdata:
        config_file = os.path.join(appdata, "openparts-kicad-plugin", "binary_path.txt")
    else:
        config_file = os.path.expanduser(
            "~/.config/openparts-kicad-plugin/binary_path.txt"
        )
    if os.path.isfile(config_file):
        with open(config_file, "r", encoding="utf-8") as f:
            path = f.read().strip()
            if path:
                return path

    return DEFAULT_BINARY_PATH


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
        binary = resolve_binary_path()
        if not os.path.isfile(binary):
            pcbnew.Refresh()
            wx_message_box(
                "openparts-kicad-plugin binary not found at:\n{}\n\n"
                "Re-run install.sh/install.ps1, build it yourself with "
                "`cargo build --release`, or set the "
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
