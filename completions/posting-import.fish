# posting-import fish completion

complete -c posting-import -s a -l app -r -f -a "postman insomnia bruno" \
    -d "Source application type"
complete -c posting-import -s s -l source -r -F \
    -d "Path to the source collection file or directory"
complete -c posting-import -s t -l target -r -F \
    -d "Output directory for the Posting collection"
complete -c posting-import -s w -l overwrite -n -f \
    -d "Overwrite existing files"
complete -c posting-import -s v -l verbose -n -f \
    -d "Verbose output"
complete -c posting-import -s n -l dry-run -n -f \
    -d "Don't write output"
complete -c posting-import -s c -l name -r \
    -d "Collection name"
complete -c posting-import -l list-sources -n -f \
    -d "List supported source formats and exit"
complete -c posting-import -l format -r -f -a "text json yaml" \
    -d "Output format for collection info"
complete -c posting-import -s h -l help -n -f \
    -d "Print help"
complete -c posting-import -s V -l version -n -f \
    -d "Print version"
