Output "{{OUT}}/term.mp4"
Set Shell "zsh"
Set FontFamily "Menlo"
Set FontSize 20
Set Width 960
Set Height 880
Set Padding 18
Set Framerate 30
Set TypingSpeed 45ms
Set Theme { "name": "Sodium", "black": "#141412", "red": "#ff6b57", "green": "#8cc79a", "yellow": "#ffd400", "blue": "#7f9fc4", "magenta": "#c49fc4", "cyan": "#7fc4bf", "white": "#c4c0b6", "brightBlack": "#736d61", "brightRed": "#ff8a79", "brightGreen": "#a8dcb3", "brightYellow": "#ffe14d", "brightBlue": "#a3bfe0", "brightMagenta": "#dcbfdc", "brightCyan": "#a3dcd7", "brightWhite": "#f5f3ee", "background": "#141412", "foreground": "#f5f3ee", "selection": "#3a3107", "cursor": "#ffd400" }

Hide
Type "unset ANTHROPIC_API_KEY; export DISABLE_COST_WARNINGS=1; export PS1='$ '; cd {{CWD}}" Enter
Type `claude() { command claude --model claude-sonnet-5-5 --mcp-config {{OUT}}/mcp.json --strict-mcp-config --allowedTools 'mcp__phonedriver__*' --tools ToolSearch --append-system-prompt 'You operate a phone with the phonedriver MCP tools. Use only those tools.' "$@"; }` Enter
Type "clear" Enter
Sleep 500ms
Type "python3 -c 'import time;print(time.time())' > {{OUT}}/term-start.txt; clear" Enter
Sleep 300ms
Show
Sleep 1s
Type "claude"
Sleep 400ms
Enter
Wait+Screen@30s /\/effort/
Sleep 1.5s
Type `{{PROMPT}}`
Sleep 600ms
Enter
Sleep {{WAIT}}s
