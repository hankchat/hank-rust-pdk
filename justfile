chooser := "grep -v choose | fzf --tmux"
# Display this list of available commands
@list:
    just --justfile "{{ source_file() }}" --list

alias c := choose
# Open an interactive chooser of available commands
[no-exit-message]
@choose:
    just --justfile "{{ source_file() }}" --chooser "{{ chooser }}" --choose 2>/dev/null

alias e := edit
# Edit the justfile
@edit:
    $EDITOR "{{ justfile() }}"

publish version="patch":
    cargo release version {{ version }} --no-confirm --execute
    cargo release commit --no-confirm --execute
    cargo release tag --no-confirm --execute
    cargo release push --no-confirm --execute
    cargo release publish --no-confirm --execute
