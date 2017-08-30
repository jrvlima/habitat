---
title: Environment variables
---

# Environment Variables

This is a list of all environment variables that can be used to modify the operation of Habitat.

| Variable | Context | Default | Description |
|----------|---------|---------|-------------|
| `HAB_AUTH_TOKEN` | build system | no default | Authorization token used to perform privileged operations against the depot, e.g. uploading packages or keys.
| `HAB_BINLINK_DIR` | build system | `/hab/bin` | Allows you to change the target directory for the symlink created when you run `hab pkg binlink`. The default value is already included in the `$PATH` variable inside the studio. |
| `HAB_CACHE_KEY_PATH` | build system, supervisor | `/hab/cache/keys` if running as root; `$HOME/.hab/cache/keys` if running as non-root | Cache directory for origin signing keys |
| `HAB_DEPOT_URL` | build system, supervisor | `https://willem.habitat.sh/v1/depot` | The depot (or channel in the depot) used by the Habitat build system or supervisor |
| `HAB_DOCKER_OPTS` | build system | no default | When running a studio on a platform that uses Docker (MacOS), additional command line options to pass to the `docker` command. |
| `HAB_NOCOLORING` | build system | no default | If set to the lowercase string `"true"` this environment variable will unconditionally disable text coloring where possible |
| `HAB_NONINTERACTIVE` | build system | no default | If set to the lowercase string `"true"` this environment variable will unconditionally disable interactive progress bars (i.e. "spinners") where possible |
| `HAB_ORG` | supervisor | no default | Organization to use when running with [service group encryption](/docs/run-packages-security/#service-group-encryption)
| `HAB_ORIGIN` | build system | no default | Origin used to build packages. The signing key for this origin is passed to the build system. |
| `HAB_ORIGIN_KEYS` | build system | no default | Comma-separated list of origin keys to automatically share with the build system |
| `HAB_RING` | supervisor | no default | The ring used by the supervisor when running with [wire encryption](/docs/run-packages-security/#wire-encryption) |
| `HAB_RING_KEY` | supervisor | no default | The name of the ring key when running with [wire encryption](/docs/run-packages-security/#wire-encryption) |
| `HAB_STUDIOS_HOME` | build system | `/hab/studios` if running as root; `$HOME/.hab/studios` if running as non-root | Directory in which to create build studios |
| `HAB_STUDIO_ROOT` | build system | no default | Root of the current studio under `$HAB_STUDIOS_HOME`. Infrequently overridden. |
| `HAB_STUDIO_SUP` | build system | no default | Used to customize the arguments passed to an automatically launched Supervisor, or to disable the automatic launching by setting it to `false`, `no`, or `0`. |
| `HAB_UPDATE_STRATEGY_FREQUENCY_MS` | supervisor | 60000 | Frequency of milliseconds to check for updates when running with an [update strategy](/docs/run-packages-update-strategy) |
| `HAB_USER` | supervisor | no default | User key to use when running with [service group encryption](/docs/run-packages-security/#service-group-encryption) |
| `HAB_NON_ROOT` | supervisor | no default | Enable the supervisor to install and run packages as non-root user. *Warning* it is highly reckomended not to use this setting except under very specific circumstances due to associated security risks. |
| `http_proxy` | build system, supervisor | no default | A URL for a local HTTP proxy server optionally supporting basic authentication |
| `https_proxy` | build system, supervisor | no default | A URL for a local HTTPS proxy server optionally supporting basic authentication |
| `no_proxy` | build system, supervisor | no default | A comma-separated list of domain exclusions for the `http_proxy` and `https_proxy` environment variables |

# Customizing Studio

When you enter a studio, Habitat will attempt to locate `/src/.studiorc` and
source it.  Think `~/.bashrc`. This file can be used to export any
environment variables like the ones you see above as well as any other shell
customizations to help you develop your plans from within the studio.

To use this feature, place a `.studiorc` in the current working directory
where you will run `hab studio enter`.
