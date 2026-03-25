# posting-import bash completion

_posting_importer() {
    local cur prev words cword
    _init_completion || return

    local -A OPTS=(
        [a]="--app"
        [s]="--source"
        [t]="--target"
        [w]="--overwrite"
        [v]="--verbose"
        [n]="--dry-run"
        [c]="--name"
        [h]="--help"
        [V]="--version"
    )

    case "$cur" in
        -*)
            COMPREPLY=($(compgen -W "${OPTS[*]}" -- "$cur"))
            ;;
        *)
            if [[ "$prev" == "--app" ]]; then
                COMPREPLY=($(compgen -W "postman insomnia bruno" -- "$cur"))
            elif [[ "$prev" == "--format" ]]; then
                COMPREPLY=($(compgen -W "text json yaml" -- "$cur"))
            elif [[ "$prev" == "--source" || "$prev" == "--target" ]]; then
                _filedir
            else
                COMPREPLY=($(compgen -W "${OPTS[*]}" -- "$cur"))
            fi
            ;;
    esac
}

complete -F _posting_importer posting-import
