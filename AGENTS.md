# Viboceros

This project aims to reimplement Rhinoceros 3D in a cross-platform, open source way using the Rust programming language.

- Start out with a clear modular architecture.
- Use wgpu and egui for the UI.
- You may need to implement a rock-solid battle-tested core geometry module. Pay extreme attention to correctness. Use nalgebra and faer for the math.
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
  - Our geometry kernel should not be significantly slower than the official Rhino.
  - Our copy of Rhino 8 is running via FEX amd64 to arm translation, so it is already significantly slower than the original version. We should beat it in performance.

Commit and push your code whenever you make progress.
Maintain a concise README.md with a brief description and documentation and instructions to set it up and run it.
Do not modify this AGENTS.md.
