# Source this on a machine where webkit2gtk-4.1 is not installed system-wide
# but has been extracted into ~/.local/opt/webkit (see CLAUDE.md, "Building
# without root"). Harmless anywhere else.
#
#   . tools/dev-env.sh
prefix=$HOME/.local/opt/webkit
if [ -d "$prefix/usr/lib/pkgconfig" ]; then
    export PKG_CONFIG_PATH="$prefix/usr/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    export LD_LIBRARY_PATH="$prefix/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export WEBKIT_EXEC_PATH="$prefix/usr/lib/webkit2gtk-4.1"
    export WEBKIT_INJECTED_BUNDLE_PATH="$prefix/usr/lib/webkit2gtk-4.1/injected-bundle"
fi
export PATH="$HOME/.local/bin:$PATH"
