# Record the README GIF with https://github.com/charmbracelet/vhs:
#
#   cargo build --release && vhs demo.tape
#
# It works in a throwaway demo repository (scripts/demo-repo.sh), never in
# the folder you run it from.
Output demo.gif
Set FontSize 15
Set Width 1400
Set Height 820
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms

Hide
Type "export PATH=$PWD/target/release:$PATH && scripts/demo-repo.sh /tmp/acme-app >/dev/null && cd /tmp/acme-app && clear" Enter
Sleep 1s
Show

Type "canopy" Sleep 300ms Enter
Sleep 2.5s
# Changes: the diff of src/main.rs, highlighted.
Type "2" Sleep 1.5s
Enter Sleep 1s
Down Sleep 300ms
Down Sleep 300ms
Down Sleep 300ms
Space Sleep 1.2s
# Stage the whole file, and commit it.
Escape Sleep 500ms
Space Sleep 1s
Type "c" Sleep 600ms
Type "Ignore case and punctuation" Sleep 800ms
Enter Sleep 1.5s
# History, with the feature branch and the tag.
Type "3" Sleep 2.5s
# The command palette.
Type ":" Sleep 500ms
Type "rebase" Sleep 1.5s
Escape Sleep 500ms
Type "?" Sleep 2s
Escape Sleep 500ms
Type "q"
Sleep 500ms
