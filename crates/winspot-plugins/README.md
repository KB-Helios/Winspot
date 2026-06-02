# winspot-plugins

Plugin manifests and the registry that loads them for the Winspot search backend.

This crate defines the plugin **manifest** format, validates manifests, emits typed
validation reports, and exposes a [`PluginRegistry`] that the daemon uses to
surface plugins as search results. It covers Winspot's phase-1 plugin model:
declarative, in-process Rust plugin identities. WASM sandboxing and a plugin
marketplace are intentionally out of scope for now.

## Manifest format

A plugin is described by a single JSON file. Field names are camelCase:

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "capabilities": ["ClipboardWrite"],
  "enabled": true
}
```

| Field          | Type       | Required | Default | Description |
| -------------- | ---------- | -------- | ------- | ----------- |
| `id`           | string     | yes      | —       | Stable identifier. Becomes the result id `plugin:<id>`. |
| `name`         | string     | yes      | —       | Display title shown in search results. |
| `capabilities` | string[]   | no       | `[]`    | Capabilities the plugin needs (see below). |
| `enabled`      | bool       | no       | `true`  | Set to `false` to keep the manifest on disk but hide it. |

### Allowed capabilities

`capabilities` entries must be one of the following (PascalCase) values, mirroring
`winspot_core::ActionCapability`:

- `ClipboardWrite`
- `FilesystemRead`
- `FilesystemWrite`
- `ProcessExecution`
- `ProcessInspection`
- `ShellExecution`
- `PluginExecution`

Capabilities are enforced by the action executor (`winspot-actions`): an action is
only run if its capability is allowed. Declare the minimum set your plugin needs.
In V1, user-authored manifests are search-only even when they declare executable
capabilities; the validator reports those declarations as warnings.

## Validation rules

Every manifest is validated before it is registered (see
[`PluginManifest::validate`]). A manifest is **rejected** (and skipped, with a
diagnostic on stderr — it never crashes the daemon) when:

- `id` is empty/whitespace.
- `id` is longer than `MAX_PLUGIN_ID_LENGTH` (64) or contains characters outside
  `[a-z0-9_-]`, or does not start with a lowercase letter or digit.
- `name` is empty/whitespace.
- the same capability is declared more than once.
- the `id` duplicates an already-registered plugin (including a built-in). The
  first registration wins; the duplicate is dropped so a user plugin can never
  silently shadow a built-in.

Validation reports keep both accepted and rejected entries. Unknown manifest
fields are warnings, not errors, so forward-looking manifests remain compatible
while still surfacing diagnostics.

## Validation CLI

Run the plugin validation stage directly with:

```powershell
cargo run -p winspot-pluginctl -- validate --plugins-dir crates\winspot-plugins\tests\fixtures\valid --format json
```

Use `--format human` for grouped text output. The command exits `0` when the
report contains no errors and `1` when any manifest has an error-severity issue.

## Where manifests are loaded from

The daemon seeds the registry with the built-in plugins, then merges any valid
`*.json` manifests found in the plugins directory:

- **Installed:** `%LOCALAPPDATA%\Winspot\plugins`
- **Portable** (a `Winspot.portable` marker sits next to the executable):
  `<exe dir>\plugins`

A missing directory is fine — you simply get the built-ins. The directory can be
overridden programmatically via `PipeConfig.plugins_dir`.

## Built-in plugins

`built_in_plugin_manifests()` provides the always-present identities:

| id                | name            | capabilities       |
| ----------------- | --------------- | ------------------ |
| `calculator`      | Calculator      | `ClipboardWrite`   |
| `terminal`        | Terminal        | `ProcessExecution` |
| `clipboard`       | Clipboard       | `ClipboardWrite`   |
| `unit-conversion` | Unit Conversion | `ClipboardWrite`   |

## Example: add a user plugin

1. Create `%LOCALAPPDATA%\Winspot\plugins\notes.json`:

   ```json
   { "id": "notes", "name": "Notes", "capabilities": ["ClipboardWrite"] }
   ```

2. Restart Winspot. Type `notes` in the launcher — the plugin appears as a result.

## Testing

```
cargo test -p winspot-plugins
cargo test -p winspot-pluginctl
```
