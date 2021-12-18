# `rlr`: a pixel screen ruler

![./logo.png](./logo.png)

Rust + GTK interactive pixel screen ruler with protractor mode.

## demo

<table>
<tr><td colspan=2 align="center"><kbd>

![./demo.png](./demo.png)
</kbd></tr><tr><td><kbd>

![./demo.gif](./demo.gif)

</kbd></td><td><kbd>

![./demo_move.gif](./demo_move.gif)

</kbd></td></tr></table>


## use

- Quit with `q` or `Ctrl-Q`.
- Click to drag.
- Press `r` to rotate 90 degrees.
- Press `p` to toggle protractor mode.
- Press `f` or `<Space>` to toggle freezing the measurements.
- Press `Control_L` and drag the angle base side to rotate it in protractor mode.
- Press `Control_L` continuously to disable precision (measurements will snap to nearest integer).
- Press `+` to increase size.
- Press `-` to decrease size.
- Press `Up`, `Down`, `Left`, `Right` to move window position by 10 pixels. Also hold down `Control_L` to move by 1 pixel.

## build

```shell
cargo build --release
```
