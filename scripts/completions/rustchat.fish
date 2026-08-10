complete -c rustchat -s m -l model -x -a "(rustchat --list-models)" -d 'Select a LLM model' -r
complete -c rustchat -l prompt -d 'Use the system prompt'
complete -c rustchat -s r -l role -x -a "(rustchat --list-roles)" -d 'Select a role' -r
complete -c rustchat -s s -l session -x  -a "(rustchat --list-sessions)" -d 'Start or join a session' -r
complete -c rustchat -l empty-session -d 'Ensure the session is empty'
complete -c rustchat -l save-session -d 'Ensure the new conversation is saved to the session'
complete -c rustchat -s a -l agent -x  -a "(rustchat --list-agents)" -d 'Start a agent' -r
complete -c rustchat -l agent-variable -d 'Set agent variables'
complete -c rustchat -l rag -x  -a"(rustchat --list-rags)" -d 'Start a RAG' -r
complete -c rustchat -l rebuild-rag -d 'Rebuild the RAG to sync document changes'
complete -c rustchat -l macro -x  -a"(rustchat --list-macros)" -d 'Execute a macro' -r
complete -c rustchat -l serve -d 'Serve the LLM API and WebAPP'
complete -c rustchat -s e -l execute -d 'Execute commands in natural language'
complete -c rustchat -s c -l code -d 'Output code only'
complete -c rustchat -s f -l file -d 'Include files, directories, or URLs' -r -F
complete -c rustchat -s S -l no-stream -d 'Turn off stream mode'
complete -c rustchat -l dry-run -d 'Display the message without sending it'
complete -c rustchat -l info -d 'Display information'
complete -c rustchat -l sync-models -d 'Sync models updates'
complete -c rustchat -l list-models -d 'List all available chat models'
complete -c rustchat -l list-roles -d 'List all roles'
complete -c rustchat -l list-sessions -d 'List all sessions'
complete -c rustchat -l list-agents -d 'List all agents'
complete -c rustchat -l list-rags -d 'List all RAGs'
complete -c rustchat -l list-macros -d 'List all macros'
complete -c rustchat -s h -l help -d 'Print help'
complete -c rustchat -s V -l version -d 'Print version'
