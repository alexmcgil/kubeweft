# Glossary

All terms describe an early experimental system; APIs, manifests, and protocols that use them are unstable.

## Application

A logical user-facing program that can have implementations or placements on different devices.

## Adapter

Platform-specific code that implements a capability using a local mechanism while keeping that mechanism out of core dependencies.

## Agent

A process on a device that coordinates local capabilities and adapters and executes authorized work locally.

## Capability

An extensible namespaced statement of something a device can do, such as `core.files.read` or `core.application.launch`.

## Control plane

The coordination path for discovery, capability advertisement, authorization, placement, job starts, route negotiation, state tracking, and explanations.

## Controller

A control-plane role that coordinates cluster state and decisions; it should not unnecessarily proxy large application data.

## Data plane

The direct path carrying application data, such as a media stream or file transfer, between suitable endpoints.

## Device

A physical device participating in a Kubeweft cluster.

## Job

Work owned by the cluster rather than a particular device.

## Manifest

A human-facing declarative configuration document validated against a versioned schema and later converted to typed internal models.

## Planner

The higher-level boundary reserved for capability, application placement, and route planning.

## Plugin

A future separately running process that extends an agent through IPC, not an in-process Rust ABI library.

## Policy

Authorization and placement constraints governing capability access and work.

## Resource

A device asset such as CPU, RAM, GPU, storage, display, microphone, or network capacity.

## Scheduler

The pure component that transforms cluster state and a job request into a plan; it never executes operating-system actions.

## Service

A logical long-running or networked function that may have implementations on different devices.
