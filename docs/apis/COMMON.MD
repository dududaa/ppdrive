# COMMNON OPTIONS
This is a documentation for options shared across multple APIs. It is important to have this seperated in order to keep updates synchronized.

#### CreateBucketOptions
- `label`:
    - required: `true`
    - type: `String`
    - description: A descriptive label for the bucket.
- `root_path`:
    - required: `false`
    - type: `String`
    - description: If this option is specified, all [Assets](../../TERMINOLOGIES.MD#asset) uploaded to PPDRIVE will have paths relative to the specified path.
- `size`:
    - required: `false`
    - type: `float64`
    - description: If specified, eqauls total size of [Assets](../../TERMINOLOGIES.MD#asset) that can be upload to the bucket. Think of this as an upload limit. It requires the `root_path` option to be set.
- `accepts`:
    - required: `false`
    - type: `String`
    - description: The file mime type(s) that can be uploaded to the bucket. There different formats for specifying acceptable mime types:
        - `*` is the default and means all mime types are accepted.
        - `custom` means a selection of mimetypes manually specified by a user. Acceptable format should start with "custom" keyword followed by a colon ":" and comma seprated mimetypes. Example, `custom:application/zip,audio/3gpp`
        - You can specify a group of mimes using the `filetype` they belong to (e.g, "audio", "video", "application"...etc).
        - You can also specify a *list* of comma seprated groups e.g, "audio,video,application".
- `public`:
    - required: `false`
    - type: `Bool`
    - description: This will be the default vibility set for any asset uploaded to this bucket if no visibility is set for the asset.