# Protocol Buffers

`kubeweft.v1` defines transport-neutral messages shared by future Rust and
Kotlin consumers. The contracts deliberately define no RPC services or
transport binding.

Field numbers are part of the wire contract. When removing a field, reserve its
number and never reuse it.
