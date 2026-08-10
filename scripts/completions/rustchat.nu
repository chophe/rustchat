module completions {

  def "nu-complete rustchat completions" [] {
    [ "bash" "zsh" "fish" "powershell" "nushell" ]
  }

  def "nu-complete rustchat model" [] {
    ^rustchat --list-models |
    | lines 
    | parse "{value}" 
  }

  def "nu-complete rustchat role" [] {
    ^rustchat --list-roles |
    | lines 
    | parse "{value}" 
  }

  def "nu-complete rustchat session" [] {
    ^rustchat --list-sessions |
    | lines 
    | parse "{value}" 
  }

  def "nu-complete rustchat agent" [] {
    ^rustchat --list-agents |
    | lines 
    | parse "{value}" 
  }

  def "nu-complete rustchat rag" [] {
    ^rustchat --list-rags |
    | lines 
    | parse "{value}" 
  }

  def "nu-complete rustchat macro" [] {
    ^rustchat --list-macros |
    | lines 
    | parse "{value}" 
  }

  export extern rustchat [
    --model(-m): string@"nu-complete rustchat model"      # Select a LLM model
    --prompt                                            # Use the system prompt
    --role(-r): string@"nu-complete rustchat role"        # Select a role
    --session(-s): string@"nu-complete rustchat session"  # Start or join a session
    --empty-session                                     # Ensure the session is empty
    --save-session                                      # Ensure the new conversation is saved to the session
    --agent(-a): string@"nu-complete rustchat agent"      # Start a agent
    --agent-variable                                    # Set agent variables
    --rag: string@"nu-complete rustchat rag"              # Start a RAG
    --rebuild-rag                                       # Rebuild the RAG to sync document changes
    --macro: string@"nu-complete rustchat macro"          # Execute a macro
    --serve                                             # Serve the LLM API and WebAPP
    --execute(-e)                                       # Execute commands in natural language
    --code(-c)                                          # Output code only
    --file(-f): string                                  # Include files, directories, or URLs
    --no-stream(-S)                                     # Turn off stream mode
    --dry-run                                           # Display the message without sending it
    --info                                              # Display information
    --sync-models                                       # Sync models updates
    --list-models                                       # List all available chat models
    --list-roles                                        # List all roles
    --list-sessions                                     # List all sessions
    --list-agents                                       # List all agents
    --list-rags                                         # List all RAGs
    --list-macros                                       # List all macros
    ...text: string                                     # Input text
    --help(-h)                                          # Print help
    --version(-V)                                       # Print version
  ]

}

export use completions *
