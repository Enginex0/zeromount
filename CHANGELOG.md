# Changelog

## v2.0.146-dev

- Detect and uninstall conflicting metamodules during installation

## v2.0.144-dev

- Change safe mode trigger from volume-down to volume-up + volume-down combo (fixes fastboot key conflict)
- Add persistent recovery lockout so guard recovery survives boot-completed cleanup
- Add `guard clear-lockout` CLI subcommand and WebUI lockout banner with clear button
- Add diagnostic kmsg logging when guard marker recording fails

## v2.0.143-dev

- Add i18n support for 36 languages in WebUI and daemon description strings

## v2.0.142-dev

- Initial release on zeromount repository
