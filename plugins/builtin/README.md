# Built-in plugin boundary

This directory is reserved for plugins shipped with Kubeweft but still run as separate processes. Such plugins may depend on the plugin SDK and their own adapters. They must not be linked into core crates or redefine scheduler, policy, or protocol ownership.
