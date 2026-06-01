# winspot-plugins

Plugin manifests and the registry that loads them for the Winspot search backend.

This crate defines the plugin **manifest** format, validates manifests, and exposes
a [`PluginRegistry`] that the daemon uses to surface plugins as search results. It
covers Winspot's phase-1 plugin model: declarative, in-process Rust plugin
identities. WASM sandboxing and a plugin marketplace are intentionally out of scope
for now.

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
```
