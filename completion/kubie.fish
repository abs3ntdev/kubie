# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_kubie_global_optspecs
	string join \n h/help V/version
end

function __fish_kubie_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_kubie_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_kubie_using_subcommand
	set -l cmd (__fish_kubie_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c kubie -n "__fish_kubie_needs_command" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_needs_command" -s V -l version -d 'Print version'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "ctx" -d 'Spawn a shell in the given context. The shell is isolated from other shells. Kubie shells can be spawned recursively without any issue'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "ns" -d 'Change the namespace in which the current shell operates. The namespace change does not affect other shells'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "info" -d 'View info about the current kubie shell, such as the context name and the current namespace'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "exec" -d 'Execute a command inside of the given context and namespace'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "export" -d 'Prints the path to an isolated configuration file for a context and namespace'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "lint" -d 'Check the Kubernetes config files for issues'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "edit" -d 'Edit the given context'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "edit-config" -d 'Edit kubie\'s config file'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "update" -d 'Check for a Kubie update and replace Kubie\'s binary if needed. This function can ask for sudo-mode'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "delete" -d 'Delete a context. Automatic garbage collection will be performed. Dangling users and clusters will be removed'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "sessions" -d 'List all active kubie sessions'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "generate-completion" -d 'Generate a completion script. Enable completion using `source <(kubie generate-completion)`. This can be added to your shell\'s configuration file to enable completion automatically'
complete -c kubie -n "__fish_kubie_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c kubie -n "__fish_kubie_using_subcommand ctx" -s n -l namespace -d 'Specify in which namespace of the context the shell is spawned' -r
complete -c kubie -n "__fish_kubie_using_subcommand ctx" -s f -l kubeconfig -d 'Specify files from which to load contexts instead of using the installed ones' -r
complete -c kubie -n "__fish_kubie_using_subcommand ctx" -s r -l recursive -d 'Enter the context by spawning a new recursive shell'
complete -c kubie -n "__fish_kubie_using_subcommand ctx" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand ns" -s r -l recursive -d 'Enter the namespace by spawning a new recursive shell'
complete -c kubie -n "__fish_kubie_using_subcommand ns" -s u -l unset -d 'Unsets the namespace in the currently active context'
complete -c kubie -n "__fish_kubie_using_subcommand ns" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand info; and not __fish_seen_subcommand_from ctx ns depth help" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand info; and not __fish_seen_subcommand_from ctx ns depth help" -f -a "ctx" -d 'Get the current shell\'s context name'
complete -c kubie -n "__fish_kubie_using_subcommand info; and not __fish_seen_subcommand_from ctx ns depth help" -f -a "ns" -d 'Get the current shell\'s namespace name'
complete -c kubie -n "__fish_kubie_using_subcommand info; and not __fish_seen_subcommand_from ctx ns depth help" -f -a "depth" -d 'Get the current depth of contexts'
complete -c kubie -n "__fish_kubie_using_subcommand info; and not __fish_seen_subcommand_from ctx ns depth help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from ctx" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from ns" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from depth" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from help" -f -a "ctx" -d 'Get the current shell\'s context name'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from help" -f -a "ns" -d 'Get the current shell\'s namespace name'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from help" -f -a "depth" -d 'Get the current depth of contexts'
complete -c kubie -n "__fish_kubie_using_subcommand info; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c kubie -n "__fish_kubie_using_subcommand exec" -l context-headers -d 'Overrides behavior.print_context_in_exec in Kubie settings file' -r -f -a "auto\t''
always\t''
never\t''"
complete -c kubie -n "__fish_kubie_using_subcommand exec" -s e -l exit-early -d 'Exit early if a command fails when using a wildcard context'
complete -c kubie -n "__fish_kubie_using_subcommand exec" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand export" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand lint" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand edit" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand edit-config" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand update" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand delete" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand sessions" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand generate-completion" -s h -l help -d 'Print help'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "ctx" -d 'Spawn a shell in the given context. The shell is isolated from other shells. Kubie shells can be spawned recursively without any issue'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "ns" -d 'Change the namespace in which the current shell operates. The namespace change does not affect other shells'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "info" -d 'View info about the current kubie shell, such as the context name and the current namespace'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "exec" -d 'Execute a command inside of the given context and namespace'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "export" -d 'Prints the path to an isolated configuration file for a context and namespace'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "lint" -d 'Check the Kubernetes config files for issues'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "edit" -d 'Edit the given context'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "edit-config" -d 'Edit kubie\'s config file'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "update" -d 'Check for a Kubie update and replace Kubie\'s binary if needed. This function can ask for sudo-mode'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "delete" -d 'Delete a context. Automatic garbage collection will be performed. Dangling users and clusters will be removed'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "sessions" -d 'List all active kubie sessions'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "generate-completion" -d 'Generate a completion script. Enable completion using `source <(kubie generate-completion)`. This can be added to your shell\'s configuration file to enable completion automatically'
complete -c kubie -n "__fish_kubie_using_subcommand help; and not __fish_seen_subcommand_from ctx ns info exec export lint edit edit-config update delete sessions generate-completion help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c kubie -n "__fish_kubie_using_subcommand help; and __fish_seen_subcommand_from info" -f -a "ctx" -d 'Get the current shell\'s context name'
complete -c kubie -n "__fish_kubie_using_subcommand help; and __fish_seen_subcommand_from info" -f -a "ns" -d 'Get the current shell\'s namespace name'
complete -c kubie -n "__fish_kubie_using_subcommand help; and __fish_seen_subcommand_from info" -f -a "depth" -d 'Get the current depth of contexts'
