# Pomodoro Time Chamber

A terminal pomodoro timer with a task list, written in Rust with [ratatui](https://ratatui.rs).
It is scriptable with Lua plugins.

<img width="990" height="714" alt="image" src="https://github.com/user-attachments/assets/ab3a9ab6-1329-4299-b0c5-3fc689f3099d" />

## Install

On Arch Linux, from the [AUR](https://aur.archlinux.org/packages/pomodoro-time-chamber) (builds from source):

```sh
yay -S pomodoro-time-chamber   # or paru, or makepkg from the AUR git repo
```

Otherwise, download the Linux (x86_64) binary from the [latest release](../../releases/latest):

```sh
tar -xzf ptc-v*-x86_64-linux.tar.gz
install -m755 ptc-v*-x86_64-linux/ptc ~/.local/bin/ptc
```

Each release also has a `.sha256` file to verify the download. Or build it yourself (next section).

## Build and run

```sh
cargo build --release
./target/release/ptc      # or: cargo run
```

The binary is called `ptc`. Lua 5.4 is compiled into it, so you don't need Lua installed (you do
need a C compiler to build).

## Default hotkeys

| Key | Action |
| --- | --- |
| `s` | Start / stop the pomodoro (also starts a break that is waiting) |
| `p` | Pause / resume (only while a pomodoro or break is running) |
| `b` | Skip the break (only during a break) |
| `a` | Add a task (`Enter` saves, `Esc` cancels) |
| `e` | Edit the selected task |
| `x` | Toggle the selected task as done |
| `d` | Delete the selected task |
| `c` / `C` | Clear completed tasks / clear all tasks |
| `+` / `-` | Raise / lower the selected task's estimated pomodoros (minimum 1) |
| `j` / `k` | Select next / previous task |
| `Ctrl+j` / `Ctrl+k` | Move the selected task down / up |
| `q` | Quit |

The hotkey list in the sidebar always shows the keys currently in use, so rebinding a key updates it.

Tasks are saved automatically (see [Persistence](#persistence)).

## Configuration

There is no config file yet. You configure the app from a Lua plugin with `ptc.config`, which
**merges** a partial table over the defaults. Only the options you set change.

Create a plugin file, for example `~/.config/ptc/plugins/00-config.lua`:

```lua
ptc.config({
  theme = {
    background = "#1e1e2e",
    status_work = "magenta",
  },
  pomodoro = {
    work_minutes = 50,
    auto_start_break = true,
  },
  keyboard = {
    quit = "Ctrl+q",
    add_task = "n",
  },
})
```

You can call `ptc.config` several times; patches are applied in order. If a patch has an unknown
option or an invalid value (a typo in a color, say), the **whole patch is ignored** and an error
is printed when the app exits (see [Errors](#errors)).

### Theme

Colors are strings: a name (`"red"`, `"darkgray"`, `"lightblue"`, ...; case, spaces, `-` and `_`
are ignored), a hex value (`"#112233"`) or a 256-color index (`"208"`).

| Option | Default | What it colors |
| --- | --- | --- |
| `background` | `#1a1b26` | Whole screen / left sidebar |
| `panel_background` | `#24283b` | Tasks panel |
| `foreground` | `white` | Default text |
| `title` | `white` | The `POMODORO TIME CHAMBER` title |
| `hotkey` | `blue` | Arrow and label of a hotkey hint |
| `hotkey_key` | `yellow` | The key itself in a hotkey hint |
| `status_idle` | `darkgray` | Status bar and clock while idle |
| `status_work` | `red` | Status bar and clock while focusing |
| `status_short_break` | `green` | Status bar and clock in a short break |
| `status_long_break` | `blue` | Status bar and clock in a long break |
| `selection` | `#3b4261` | Background of the selected task |
| `scrollbar` | `gray` | Task list scrollbar |
| `tasks_status_bar` | `#2e3350` | Background of the bar at the bottom of the tasks panel |
| `tasks_status_bar_text` | `gray` | Text of that bar |
| `task_unmarked` | `#1a1b26` | Marker block of an open task |
| `task_marked` | `green` | Marker block of a done task |
| `task_marked_text` | `black` | The `x` inside a done marker |

### Keyboard

Keys are strings. Modifiers (`Ctrl+`, `Alt+`, `Shift+`) and names are case-insensitive, but a single
character is not: `"c"` and `"C"` are different keys.

Supported names: `Enter`, `Esc`, `Tab`, `Backspace`, `Space`, `Up`, `Down`, `Left`, `Right`,
`Home`, `End`, `PageUp`, `PageDown`, `Delete`, `Insert`, `F1`-`F24`. Examples: `"q"`, `"Ctrl+j"`,
`"Alt+x"`, `"Enter"`, `"+"`, `"Ctrl++"`.

| Option | Default | Action |
| --- | --- | --- |
| `quit` | `q` | Quit |
| `toggle_pomodoro` | `s` | Start / stop |
| `pause_pomodoro` | `p` | Pause / resume |
| `skip_break` | `b` | Skip the break |
| `add_task` | `a` | Add a task |
| `edit_task` | `e` | Edit the selected task |
| `toggle_done` | `x` | Toggle done |
| `delete_task` | `d` | Delete the selected task |
| `clear_completed` | `c` | Clear completed tasks |
| `clear_all` | `C` | Clear all tasks |
| `increase_estimate` | `+` | More estimated pomodoros |
| `decrease_estimate` | `-` | Fewer estimated pomodoros |
| `select_next` | `j` | Select next task |
| `select_previous` | `k` | Select previous task |
| `move_task_down` | `Ctrl+j` | Move task down |
| `move_task_up` | `Ctrl+k` | Move task up |
| `confirm_task` | `Enter` | Save the task being typed |
| `cancel_task` | `Esc` | Cancel typing |

Typing text, `Backspace` while typing and the other editing keys are fixed. In a terminal, `Ctrl+j`
works because the app runs in raw mode, where it is not turned into `Enter`.

### Pomodoro

| Option | Default | Meaning |
| --- | --- | --- |
| `work_minutes` | `25` | Length of a pomodoro (whole number) |
| `short_break_minutes` | `5` | Length of a short break |
| `long_break_minutes` | `15` | Length of a long break |
| `long_break_interval` | `4` | Every Nth pomodoro is followed by a long break |
| `auto_start_break` | `false` | Start the break by itself; if `false`, it waits for `s` |
| `notifications` | `true` | Desktop notification when a phase ends |

Numbers must be whole numbers (`30`, not `30.0`).

## Plugins

Plugins are Lua 5.4 scripts that **react to events**. They can read and change configuration and
replace the task list, but they cannot add hotkeys or UI.

> Plugins are **not sandboxed**. `io`, `os` and the rest of the standard library are available, so a
> plugin can read and write files and run commands. Only install plugins you trust.

### Where plugins live

Every `*.lua` file in `~/.config/ptc/plugins/` is loaded at startup, in file-name order (prefix
names with numbers, like `00-config.lua`, to control order). The directory is
`$XDG_CONFIG_HOME/ptc/plugins` if that variable is set. The path is also available to plugins as
`ptc.plugins_dir`.

Two built-in core plugins are always loaded first: `plugins/core_persist.lua` (saves tasks) and
`plugins/core_status_file.lua` (publishes the status file). They are good examples to read.

### The `ptc` API

| Call | Description |
| --- | --- |
| `ptc.on(event, fn)` | Register a handler. An unknown event name is an error. You can register several handlers per event, from several plugins. |
| `ptc.config(table)` | Merge a partial config (see [Configuration](#configuration)). |
| `ptc.set_tasks(list)` | Replace the task list. Does not trigger `tasks_changed`. |
| `ptc.json.encode(value)` | Lua value to a pretty-printed JSON string. |
| `ptc.json.decode(text)` | JSON string to a Lua value. |
| `ptc.data_dir` | Directory for plugin data, `~/.local/share/ptc` (created for you). |
| `ptc.plugins_dir` | Directory plugins are loaded from. |
| `ptc.status_file` | Path of the status file written by the core plugin (see [Integrations](#integrations)). |

### Events

| Event | Payload | When |
| --- | --- | --- |
| `startup` | none | Once, after all plugins are loaded and before the first draw |
| `quit` | none | Once, when the app exits |
| `tasks_changed` | list of tasks | Whenever the task list changes (add, edit, delete, reorder, estimate, done, a pomodoro being credited, ...) |
| `pomodoro_state_changed` | state table | Whenever the pomodoro starts, stops, pauses, resumes, changes phase, or a break becomes ready |

A task is a table:

```lua
{ text = "Write the README", estimated_pomodoros = 3, completed_pomodoros = 1, done = false }
```

The `pomodoro_state_changed` payload:

```lua
{
  state = "work",            -- "idle" | "work" | "short_break" | "long_break"
  previous = "idle",         -- the state before this change
  paused = false,
  waiting = false,           -- a break is set up and waiting for the start key
  completed_pomodoros = 0,   -- finished pomodoros in this session
  ends_at = 1790000000,      -- Unix time (seconds) the phase ends; only present while counting down
  remaining_seconds = 1500,  -- time left (the full phase length when idle or waiting)
}
```

Notes:

- `tasks_changed` and `pomodoro_state_changed` are not sent for the initial load at startup.
- Handlers run on the UI thread, so keep them quick. A slow handler freezes the screen.
- `ptc.set_tasks` and `ptc.config` can be called from a handler (for example from `startup`) and
  take effect right after it. Tasks loaded this way do not echo back as `tasks_changed`.
- Estimates below 1 in `ptc.set_tasks` are raised to 1. Missing fields default to
  `estimated_pomodoros = 1`, `completed_pomodoros = 0` and `done = false`; `text` is required.

### Example: run a command when a break starts

`~/.config/ptc/plugins/10-break-sound.lua`:

```lua
ptc.on("pomodoro_state_changed", function(e)
  if e.state == "short_break" or e.state == "long_break" then
    if not e.waiting and not e.paused then
      os.execute("paplay /usr/share/sounds/freedesktop/stereo/complete.oga &")
    end
  end
end)
```

(End the command with `&` so it doesn't block the UI.)

### Example: log finished pomodoros

```lua
local log_path = ptc.data_dir .. "/pomodoros.log"

ptc.on("pomodoro_state_changed", function(e)
  if e.previous == "work" and e.state ~= "work" and e.state ~= "idle" then
    local file = assert(io.open(log_path, "a"))
    file:write(os.date("%Y-%m-%d %H:%M"), " pomodoro #", e.completed_pomodoros, " finished\n")
    file:close()
  end
end)
```

### Persistence

The core plugin saves the task list to `~/.local/share/ptc/tasks.json` on every `tasks_changed` (it
writes a temporary file and renames it, so a crash can't corrupt the list) and loads it on
`startup`.

If several plugins call `ptc.set_tasks`, the last call wins. A plugin that calls it on `startup` would
overwrite the saved tasks, so use it with care.

### Errors

A plugin that fails to load, a handler that raises an error, or a rejected `ptc.config` patch never
crashes the app. The messages are collected and printed to stderr **after the app exits**, so they
don't garble the screen. If something doesn't seem to work, quit and read the output. For handlers,
the message includes the event name; for load errors, the plugin file name.

## Integrations

### Pomodoro in your Neovim statusline

1. **The status file.** The always-on core plugin `plugins/core_status_file.lua` writes the pomodoro
   state to `$XDG_RUNTIME_DIR/ptc-status.json` (or `/tmp/ptc-status.json`; set `PTC_STATUS_FILE`
   before starting ptc to change it) on startup and on every state change, and `{"state":"off"}`
   when ptc quits. The path is also available to plugins as `ptc.status_file`.

   The file looks like this. While a phase is counting down it has `ends_at`, so readers compute
   `ends_at - now` themselves and ptc doesn't need to write every second. When paused, waiting or
   idle, `ends_at` is absent and `remaining_seconds` is used.

   ```json
   {
     "state": "work",
     "paused": false,
     "waiting": false,
     "ends_at": 1790000000,
     "remaining_seconds": 1500,
     "completed_pomodoros": 2,
     "updated_at": 1789998500
   }
   ```

2. **Install the Neovim module (0.10+).** This repository is also a Neovim plugin: the module lives
   in `contrib/nvim/lua/ptc.lua`, so the plugin manager must add the `contrib/nvim` subdirectory to
   the runtimepath.

   packer.nvim:

   ```lua
   use {
     'andersondanilo/pomodoro-time-chamber',
     rtp = 'contrib/nvim',
     config = function() require('ptc').setup() end,
   }
   ```

   lazy.nvim:

   ```lua
   {
     'andersondanilo/pomodoro-time-chamber',
     config = function(plugin)
       vim.opt.rtp:append(plugin.dir .. '/contrib/nvim')
       require('ptc').setup()
     end,
   }
   ```

   Or copy `contrib/nvim/lua/ptc.lua` to `lua/ptc.lua` in your Neovim config. `setup()` re-reads the
   file and redraws the statusline every second.

3. **Put it in your statusline.** `require('ptc').status()` returns the text.

   ```lua
   -- plain statusline:
   vim.o.statusline = "%f %= %{v:lua.require'ptc'.status()}"

   -- lualine:
   require("lualine").setup({ sections = { lualine_x = { require("ptc").status } } })
   ```

   vim-airline (shown before the filetype in the right-hand section):

   ```lua
   vim.cmd [[
     function! PtcStatus()
       return luaeval("require('ptc').status()")
     endfunction
     function! PtcAirlineInit()
       call airline#parts#define_function('ptc', 'PtcStatus')
       let g:airline_section_x = airline#section#create_right(['ptc', 'filetype'])
     endfunction
     autocmd User AirlineAfterInit call PtcAirlineInit()
   ]]
   ```

   **Complete example for packer.nvim + vim-airline** (add it inside `packer.startup`, then run
   `:PackerSync`):

   ```lua
   use {
     'andersondanilo/pomodoro-time-chamber',
     rtp = 'contrib/nvim',  -- the Neovim module lives in a subdirectory of the repo
     config = function()
       require('ptc').setup()  -- re-reads the status file and redraws every second

       vim.cmd [[
         function! PtcStatus()
           return luaeval("require('ptc').status()")
         endfunction
         function! PtcAirlineInit()
           call airline#parts#define_function('ptc', 'PtcStatus')
           let g:airline_section_x = airline#section#create_right(['ptc', 'filetype'])
         endfunction
         autocmd User AirlineAfterInit call PtcAirlineInit()
       ]]
     end,
   }
   ```

   This only installs the Neovim module. The `ptc` binary still has to be built and running for the
   status file to exist.

   It shows, for example, `🍅 Focus 12:28`, `🍅 Break 03:20 (paused)` or `🍅 Break 05:00 (ready)`,
   and nothing when ptc is idle or not running (or died: a counting phase more than 5 seconds past
   its `ends_at` is ignored).

Any other program can read the same file, for example a tmux, polybar or waybar script. If you run
several ptc instances at once, give each its own `PTC_STATUS_FILE`.

## Releases

Releases are automated with [release-please](https://github.com/googleapis/release-please). It reads
the commit messages on `master`, so they must follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat: ...`, `fix: ...`, `feat!: ...` for breaking changes; `chore:`/`docs:` don't trigger a release).

1. release-please keeps a "release PR" open that bumps the version in `Cargo.toml` and updates
   `CHANGELOG.md`.
2. Merging that PR creates the tag (`vX.Y.Z`) and the GitHub release.
3. The workflow then builds the Linux binary and attaches `ptc-vX.Y.Z-x86_64-linux.tar.gz` (and its
   `.sha256`) to the release, and publishes the new version of the AUR package
   (`aur/PKGBUILD`, see [One-time setup: the AUR package](#one-time-setup-the-aur-package)).

The next version is computed from the commits since the last release: `fix:` bumps the patch
(`1.0.0` -> `1.0.1`), `feat:` the minor (`1.1.0`) and a breaking change (`feat!:` or a
`BREAKING CHANGE:` footer) the major (`2.0.0`).

The first release is `1.0.0`. The project started at `0.1.0` with history that is not in
Conventional Commits, so that version is forced by a commit with a `Release-As: 1.0.0` footer (the
`chore: release 1.0.0` commit). After it is released, versions follow the rules above.

### One-time setup: the `RELEASE_PLEASE_TOKEN` secret

release-please authenticates with a personal access token stored as the repository secret
`RELEASE_PLEASE_TOKEN` (it replaces the default token, so you don't have to enable "Allow GitHub
Actions to create and approve pull requests").

1. Create a fine-grained token: GitHub > Settings > Developer settings > Personal access tokens >
   Fine-grained tokens > Generate new token.
   - Repository access: **Only select repositories** > this repository.
   - Repository permissions: **Contents** (read and write), **Pull requests** (read and write) and
     **Issues** (read and write; release-please uses labels). Metadata (read) is added automatically.
   - Pick an expiration and set a reminder to renew it, because releases stop working when it expires.
2. Add it as a secret: repository Settings > Secrets and variables > Actions > New repository secret,
   name `RELEASE_PLEASE_TOKEN`, value the token. Or with the GitHub CLI: `gh secret set RELEASE_PLEASE_TOKEN`.
3. Re-run the latest "Release" workflow (Actions tab), or push a commit to `master`.

A classic token with the `repo` scope also works.

### One-time setup: the AUR package

The `publish-aur` job pushes `aur/PKGBUILD` (version and checksum filled in by the workflow) to the AUR
package `pomodoro-time-chamber` using an SSH key stored as the secret `AUR_SSH_PRIVATE_KEY`.

1. Create an account on [aur.archlinux.org](https://aur.archlinux.org).
2. Generate a key used only for this: `ssh-keygen -t ed25519 -C "aur-ptc" -f ~/.ssh/aur_ptc -N ""`
3. AUR > My Account > SSH Public Key: paste the contents of `~/.ssh/aur_ptc.pub`.
4. Add the **private** key as a repository secret: `gh secret set AUR_SSH_PRIVATE_KEY < ~/.ssh/aur_ptc`
   (or Settings > Secrets and variables > Actions > New repository secret).
5. The package does not need to exist beforehand: the first push to `pomodoro-time-chamber` on the AUR
   creates it.

Until `AUR_SSH_PRIVATE_KEY` exists the AUR job is skipped (a notice in the run log), and the rest of
the release is unaffected. To publish a release that already happened, run **Actions > Publish AUR
package > Run workflow** and enter its tag (e.g. `v1.0.0`).

To test the PKGBUILD locally, build from a tarball of your checkout (the PKGBUILD downloads the
`vX.Y.Z` tag, which only exists after a release). The package needs `options=('!lto')`:
makepkg's default LTO flags break linking of the bundled Lua.

## Development

```sh
cargo test    # plugin host, config merging and key parsing tests
```

Architecture notes and project conventions are in [`CLAUDE.md`](CLAUDE.md).
