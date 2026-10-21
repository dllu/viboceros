# Viboceros

This project aims to reimplement Rhinoceros 3D in a cross-platform, open source way using the Rust programming language.

- Start out with a clear modular architecture.
- Use wgpu and egui for the UI.
- Use `usd-brep`/`smlib` as much as possible. It is already included in `third_party/usd-brep`.
- We need to reimplement all the 1000+ commands in Rhinoceros.
  - It may be wise to use subagents to implement fully independent commands in parallel by using worktrees.
- We need to reimplement a basic UI with strack, osnap, and so on.
  - A primarily command-line driven interface with minimal buttons for the SmartTrack, Osnap, etc would suffice.
  - We need to implement basic layers and groups, and a basic layers pane to keep track of and organize layers.
  - Viewports should support basic functionality like wireframe, shaded, and ghosted view.
- No need to implement Rhino Render.
- To start with, we can just support 3dm, STL, and STEP files.
- You may use appropriately licensed open source McNeel software such as [OpenNURBS](https://github.com/mcneel/opennurbs) via the appropriate bindings.
  - Feel free to use other McNeel software or other open source software, translating to Rust or using bindings as necessary. I would prefer to translate C# code to Rust.
  - Place third party modules in a third_party directory.
- We have a licensed copy of Rhino 8 that may be running or you can launch it from ~/wines/prefixes/rhino/launch.sh. You may use its outputs as an oracle to guide your own independent implementation. But do not plagiarize or decompile proprietary code.
  - You may search online for open-source RhinoScript programs to use as test cases.
  - You should implement a Python api to compare against the official version of Rhino for instrumenting, testing, and debugging.
  - You should ensure that all geometry do not differ from the official Rhino by more than some epsilon.
- Aim for optimal performance.
  - Our code should not be significantly slower than the official Rhino. However, usd-brep is, by its design, likely slower. Make some attempt to avoid trivially avoidable slowness like accidentally quadratic searches, but ultimately correctness and feature-completeness are more important than performance.
  - Our copy of Rhino 8 is running via FEX amd64 to arm translation, so it is already significantly slower than the original version.

Commit and push your code whenever you make progress.
Maintain a concise README.md with a brief description and documentation and instructions to set it up and run it.

- The documentation often grows super long. As new features land, it is tempting to append to various files, such as the README. If the README, individual docs, or source files are excessively long, please refactor them as needed to restore the clean, modular, and concise nature we are going for.
- Once in a while, double check if the README still accurately reflects the current state. It may be stale. If so, update it.

Do not modify this AGENTS.md.
