const std = @import("std");
const ghostty_input = @import("ghostty_src/input.zig");
const terminal = @import("ghostty_src/terminal/main.zig");

const Allocator = std.mem.Allocator;

const TerminalHandle = struct {
    alloc: Allocator,
    terminal: terminal.Terminal,
    stream: terminal.Stream(*Handler),
    handler: Handler,
    default_fg: terminal.color.RGB,
    default_bg: terminal.color.RGB,

    fn init(alloc: Allocator, cols: u16, rows: u16) !*TerminalHandle {
        const handle = try alloc.create(TerminalHandle);
        errdefer alloc.destroy(handle);

        const t = try terminal.Terminal.init(alloc, .{
            .cols = cols,
            .rows = rows,
        });
        errdefer {
            var tmp = t;
            tmp.deinit(alloc);
        }

        handle.* = .{
            .alloc = alloc,
            .terminal = t,
            .handler = .{ .terminal = undefined },
            .stream = undefined,
            .default_fg = .{ .r = 0xFF, .g = 0xFF, .b = 0xFF },
            .default_bg = .{ .r = 0x00, .g = 0x00, .b = 0x00 },
        };
        handle.handler.terminal = &handle.terminal;
        handle.stream = terminal.Stream(*Handler).init(&handle.handler);
        handle.stream.parser.osc_parser.alloc = alloc;
        return handle;
    }

    fn deinit(self: *TerminalHandle) void {
        self.stream.deinit();
        self.terminal.deinit(self.alloc);
        self.alloc.destroy(self);
    }
};

const Handler = struct {
    terminal: *terminal.Terminal,

    pub fn print(self: *Handler, c: u21) !void {
        try self.terminal.print(c);
    }

    pub fn backspace(self: *Handler) !void {
        self.terminal.backspace();
    }

    pub fn horizontalTab(self: *Handler, count: u16) !void {
        for (0..@as(usize, count)) |_| {
            try self.terminal.horizontalTab();
        }
    }

    pub fn linefeed(self: *Handler) !void {
        try self.terminal.linefeed();
    }

    pub fn carriageReturn(self: *Handler) !void {
        self.terminal.carriageReturn();
    }

    pub fn setAttribute(self: *Handler, attr: terminal.Attribute) !void {
        try self.terminal.setAttribute(attr);
    }

    pub fn invokeCharset(
        self: *Handler,
        active: terminal.CharsetActiveSlot,
        slot: terminal.CharsetSlot,
        single: bool,
    ) !void {
        self.terminal.invokeCharset(active, slot, single);
    }

    pub fn configureCharset(self: *Handler, slot: terminal.CharsetSlot, set: terminal.Charset) !void {
        self.terminal.configureCharset(slot, set);
    }

    pub fn handleColorOperation(
        self: *Handler,
        op: terminal.osc.color.Operation,
        requests: *const terminal.osc.color.List,
        terminator: terminal.osc.Terminator,
    ) !void {
        _ = op;
        _ = terminator;

        if (requests.count() == 0) return;

        var it = requests.constIterator(0);
        while (it.next()) |req| {
            switch (req.*) {
                .set => |set| switch (set.target) {
                    .palette => |i| {
                        self.terminal.color_palette.colors[i] = set.color;
                        self.terminal.color_palette.mask.set(i);
                    },
                    else => {},
                },
                .reset => |target| switch (target) {
                    .palette => |i| {
                        self.terminal.color_palette.colors[i] = self.terminal.default_palette[i];
                        self.terminal.color_palette.mask.unset(i);
                    },
                    else => {},
                },
                .reset_palette => {
                    const mask = &self.terminal.color_palette.mask;
                    var mask_iterator = mask.iterator(.{});
                    while (mask_iterator.next()) |idx| {
                        const i: usize = idx;
                        self.terminal.color_palette.colors[i] = self.terminal.default_palette[i];
                    }
                    self.terminal.color_palette.mask = .initEmpty();
                },
                else => {},
            }
        }
    }

    pub fn setCursorLeft(self: *Handler, amount: u16) !void {
        self.terminal.cursorLeft(amount);
    }

    pub fn setCursorRight(self: *Handler, amount: u16) !void {
        self.terminal.cursorRight(amount);
    }

    pub fn setCursorDown(self: *Handler, amount: u16, carriage: bool) !void {
        self.terminal.cursorDown(amount);
        if (carriage) self.terminal.carriageReturn();
    }

    pub fn setCursorUp(self: *Handler, amount: u16, carriage: bool) !void {
        self.terminal.cursorUp(amount);
        if (carriage) self.terminal.carriageReturn();
    }

    pub fn setCursorCol(self: *Handler, col: u16) !void {
        self.terminal.setCursorPos(self.terminal.screen.cursor.y + 1, col);
    }

    pub fn setCursorRow(self: *Handler, row: u16) !void {
        self.terminal.setCursorPos(row, self.terminal.screen.cursor.x + 1);
    }

    pub fn setCursorPos(self: *Handler, row: u16, col: u16) !void {
        self.terminal.setCursorPos(row, col);
    }

    pub fn eraseDisplay(self: *Handler, mode: terminal.EraseDisplay, protected: bool) !void {
        self.terminal.eraseDisplay(mode, protected);
    }

    pub fn eraseLine(self: *Handler, mode: terminal.EraseLine, protected: bool) !void {
        self.terminal.eraseLine(mode, protected);
    }

    pub fn startHyperlink(self: *Handler, uri: []const u8, id: ?[]const u8) !void {
        try self.terminal.screen.startHyperlink(uri, id);
    }

    pub fn endHyperlink(self: *Handler) !void {
        self.terminal.screen.endHyperlink();
    }

    pub fn setMode(self: *Handler, mode: terminal.Mode, enabled: bool) !void {
        self.terminal.modes.set(mode, enabled);

        switch (mode) {
            .alt_screen_legacy => self.terminal.switchScreenMode(.@"47", enabled),
            .alt_screen => self.terminal.switchScreenMode(.@"1047", enabled),
            .alt_screen_save_cursor_clear_enter => self.terminal.switchScreenMode(.@"1049", enabled),
            else => {},
        }
    }
};

export fn ghostty_vt_terminal_new(cols: u16, rows: u16) callconv(.C) ?*anyopaque {
    const alloc = std.heap.c_allocator;
    const handle = TerminalHandle.init(alloc, cols, rows) catch return null;
    return @ptrCast(handle);
}

export fn ghostty_vt_terminal_free(terminal_ptr: ?*anyopaque) callconv(.C) void {
    if (terminal_ptr == null) return;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));
    handle.deinit();
}

export fn ghostty_vt_terminal_set_default_colors(
    terminal_ptr: ?*anyopaque,
    fg_r: u8,
    fg_g: u8,
    fg_b: u8,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
) callconv(.C) void {
    if (terminal_ptr == null) return;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));
    handle.default_fg = .{ .r = fg_r, .g = fg_g, .b = fg_b };
    handle.default_bg = .{ .r = bg_r, .g = bg_g, .b = bg_b };
}

export fn ghostty_vt_terminal_feed(
    terminal_ptr: ?*anyopaque,
    bytes: [*]const u8,
    len: usize,
) callconv(.C) c_int {
    if (terminal_ptr == null) return 1;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    var i: usize = 0;
    while (i < len) : (i += 1) {
        handle.stream.next(bytes[i]) catch return 2;
    }

    return 0;
}

export fn ghostty_vt_terminal_resize(
    terminal_ptr: ?*anyopaque,
    cols: u16,
    rows: u16,
) callconv(.C) c_int {
    if (terminal_ptr == null) return 1;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    handle.terminal.resize(
        handle.alloc,
        @as(terminal.size.CellCountInt, @intCast(cols)),
        @as(terminal.size.CellCountInt, @intCast(rows)),
    ) catch return 2;
    return 0;
}

export fn ghostty_vt_terminal_scroll_viewport(
    terminal_ptr: ?*anyopaque,
    delta_lines: i32,
) callconv(.C) c_int {
    if (terminal_ptr == null) return 1;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    handle.terminal.scrollViewport(.{ .delta = @as(isize, delta_lines) }) catch return 2;
    return 0;
}

export fn ghostty_vt_terminal_scroll_viewport_top(terminal_ptr: ?*anyopaque) callconv(.C) c_int {
    if (terminal_ptr == null) return 1;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    handle.terminal.scrollViewport(.top) catch return 2;
    return 0;
}

export fn ghostty_vt_terminal_scroll_viewport_bottom(terminal_ptr: ?*anyopaque) callconv(.C) c_int {
    if (terminal_ptr == null) return 1;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    handle.terminal.scrollViewport(.bottom) catch return 2;
    return 0;
}

export fn ghostty_vt_terminal_cursor_position(
    terminal_ptr: ?*anyopaque,
    col_out: ?*u16,
    row_out: ?*u16,
) callconv(.C) bool {
    if (terminal_ptr == null) return false;
    if (col_out == null or row_out == null) return false;
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    col_out.?.* = @intCast(handle.terminal.screen.cursor.x + 1);
    row_out.?.* = @intCast(handle.terminal.screen.cursor.y + 1);
    return true;
}

export fn ghostty_vt_terminal_dump_viewport(terminal_ptr: ?*anyopaque) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const alloc = std.heap.c_allocator;
    const slice = handle.terminal.screen.dumpStringAlloc(alloc, .{ .viewport = .{} }) catch {
        return .{ .ptr = null, .len = 0 };
    };

    return .{ .ptr = slice.ptr, .len = slice.len };
}

export fn ghostty_vt_terminal_dump_viewport_row(
    terminal_ptr: ?*anyopaque,
    row: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const pt: terminal.point.Point = .{ .viewport = .{ .x = 0, .y = row } };
    const pin = handle.terminal.screen.pages.pin(pt) orelse return .{ .ptr = null, .len = 0 };

    const alloc = std.heap.c_allocator;
    var builder = std.ArrayList(u8).init(alloc);
    errdefer builder.deinit();

    handle.terminal.screen.pages.encodeUtf8(builder.writer(), .{
        .tl = pin,
        .br = pin,
        .unwrap = false,
    }) catch return .{ .ptr = null, .len = 0 };

    const slice = builder.toOwnedSlice() catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = slice.ptr, .len = slice.len };
}

const CellStyle = extern struct {
    fg_r: u8,
    fg_g: u8,
    fg_b: u8,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
    flags: u8,
    reserved: u8,
};

export fn ghostty_vt_terminal_dump_viewport_row_cell_styles(
    terminal_ptr: ?*anyopaque,
    row: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const pt: terminal.point.Point = .{ .viewport = .{ .x = 0, .y = row } };
    const pin = handle.terminal.screen.pages.pin(pt) orelse return .{ .ptr = null, .len = 0 };
    const cells = pin.cells(.all);

    const default_fg: terminal.color.RGB = handle.default_fg;
    const default_bg: terminal.color.RGB = handle.default_bg;
    const palette: *const terminal.color.Palette = &handle.terminal.color_palette.colors;

    const alloc = std.heap.c_allocator;
    var out = std.ArrayList(u8).init(alloc);
    errdefer out.deinit();

    out.ensureTotalCapacity(cells.len * @sizeOf(CellStyle)) catch return .{ .ptr = null, .len = 0 };

    for (cells) |*cell| {
        const s = pin.style(cell);

        var fg = s.fg(.{ .default = default_fg, .palette = palette, .bold = null });
        var bg = s.bg(cell, palette) orelse default_bg;

        var flags: u8 = 0;
        if (s.flags.inverse) flags |= 0x01;
        if (s.flags.bold) flags |= 0x02;
        if (s.flags.italic) flags |= 0x04;
        if (s.flags.underline != .none) flags |= 0x08;
        if (s.flags.faint) flags |= 0x10;
        if (s.flags.invisible) flags |= 0x20;
        if (s.flags.strikethrough) flags |= 0x40;

        if (s.flags.inverse) {
            const tmp = fg;
            fg = bg;
            bg = tmp;
        }
        if (s.flags.invisible) {
            fg = bg;
        }

        const rec = CellStyle{
            .fg_r = fg.r,
            .fg_g = fg.g,
            .fg_b = fg.b,
            .bg_r = bg.r,
            .bg_g = bg.g,
            .bg_b = bg.b,
            .flags = flags,
            .reserved = 0,
        };
        out.appendSlice(std.mem.asBytes(&rec)) catch return .{ .ptr = null, .len = 0 };
    }

    const slice = out.toOwnedSlice() catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = slice.ptr, .len = slice.len };
}

const StyleRun = extern struct {
    start_col: u16,
    end_col: u16,
    fg_r: u8,
    fg_g: u8,
    fg_b: u8,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
    flags: u8,
    reserved: u8,
};

fn resolvedStyle(
    default_fg: terminal.color.RGB,
    default_bg: terminal.color.RGB,
    palette: *const terminal.color.Palette,
    s: anytype,
) struct {
    fg: terminal.color.RGB,
    bg: terminal.color.RGB,
    flags: u8,
} {
    var flags: u8 = 0;
    if (s.flags.inverse) flags |= 0x01;
    if (s.flags.bold) flags |= 0x02;
    if (s.flags.italic) flags |= 0x04;
    if (s.flags.underline != .none) flags |= 0x08;
    if (s.flags.faint) flags |= 0x10;
    if (s.flags.invisible) flags |= 0x20;
    if (s.flags.strikethrough) flags |= 0x40;

    const fg = s.fg(.{ .default = default_fg, .palette = palette, .bold = null });
    return .{ .fg = fg, .bg = default_bg, .flags = flags };
}

export fn ghostty_vt_terminal_dump_viewport_row_style_runs(
    terminal_ptr: ?*anyopaque,
    row: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const pt: terminal.point.Point = .{ .viewport = .{ .x = 0, .y = row } };
    const pin = handle.terminal.screen.pages.pin(pt) orelse return .{ .ptr = null, .len = 0 };
    const cells = pin.cells(.all);

    const default_fg: terminal.color.RGB = handle.default_fg;
    const default_bg: terminal.color.RGB = handle.default_bg;
    const palette: *const terminal.color.Palette = &handle.terminal.color_palette.colors;

    const alloc = std.heap.c_allocator;
    var out = std.ArrayList(u8).init(alloc);
    errdefer out.deinit();

    if (cells.len == 0) {
        const slice = out.toOwnedSlice() catch return .{ .ptr = null, .len = 0 };
        return .{ .ptr = slice.ptr, .len = slice.len };
    }

    var current_style_id = cells[0].style_id;
    var current_style = pin.style(&cells[0]);
    const defaults = resolvedStyle(default_fg, default_bg, palette, current_style);

    var current_flags = defaults.flags;
    var current_base_fg = defaults.fg;
    var current_inverse = current_style.flags.inverse;
    var current_invisible = current_style.flags.invisible;

    var current_bg = current_style.bg(&cells[0], palette) orelse default_bg;
    var current_fg = current_base_fg;
    if (current_inverse) {
        const tmp = current_fg;
        current_fg = current_bg;
        current_bg = tmp;
    }
    if (current_invisible) {
        current_fg = current_bg;
    }

    var current_resolved = .{ .fg = current_fg, .bg = current_bg, .flags = current_flags };
    var run_start: u16 = 1;

    var col_idx: usize = 1;
    while (col_idx < cells.len) : (col_idx += 1) {
        const cell = &cells[col_idx];
        if (cell.style_id != current_style_id) {
            const end_col: u16 = @intCast(col_idx);
            const rec = StyleRun{
                .start_col = run_start,
                .end_col = end_col,
                .fg_r = current_resolved.fg.r,
                .fg_g = current_resolved.fg.g,
                .fg_b = current_resolved.fg.b,
                .bg_r = current_resolved.bg.r,
                .bg_g = current_resolved.bg.g,
                .bg_b = current_resolved.bg.b,
                .flags = current_resolved.flags,
                .reserved = 0,
            };
            out.appendSlice(std.mem.asBytes(&rec)) catch return .{ .ptr = null, .len = 0 };

            current_style_id = cell.style_id;
            current_style = pin.style(cell);
            const resolved = resolvedStyle(default_fg, default_bg, palette, current_style);
            current_flags = resolved.flags;
            current_base_fg = resolved.fg;
            current_inverse = current_style.flags.inverse;
            current_invisible = current_style.flags.invisible;

            run_start = @intCast(col_idx + 1);

            const bg_cell = current_style.bg(cell, palette) orelse default_bg;
            var fg_cell = current_base_fg;
            var bg = bg_cell;
            if (current_inverse) {
                const tmp = fg_cell;
                fg_cell = bg;
                bg = tmp;
            }
            if (current_invisible) {
                fg_cell = bg;
            }

            current_resolved = .{ .fg = fg_cell, .bg = bg, .flags = current_flags };
            continue;
        }

        const bg_cell = current_style.bg(cell, palette) orelse default_bg;
        var fg_cell = current_base_fg;
        var bg = bg_cell;
        if (current_inverse) {
            const tmp = fg_cell;
            fg_cell = bg;
            bg = tmp;
        }
        if (current_invisible) {
            fg_cell = bg;
        }

        const same = fg_cell.r == current_resolved.fg.r and fg_cell.g == current_resolved.fg.g and fg_cell.b == current_resolved.fg.b and
            bg.r == current_resolved.bg.r and bg.g == current_resolved.bg.g and bg.b == current_resolved.bg.b and
            current_flags == current_resolved.flags;
        if (same) continue;

        const end_col: u16 = @intCast(col_idx);
        const rec = StyleRun{
            .start_col = run_start,
            .end_col = end_col,
            .fg_r = current_resolved.fg.r,
            .fg_g = current_resolved.fg.g,
            .fg_b = current_resolved.fg.b,
            .bg_r = current_resolved.bg.r,
            .bg_g = current_resolved.bg.g,
            .bg_b = current_resolved.bg.b,
            .flags = current_resolved.flags,
            .reserved = 0,
        };
        out.appendSlice(std.mem.asBytes(&rec)) catch return .{ .ptr = null, .len = 0 };

        run_start = @intCast(col_idx + 1);
        current_resolved = .{ .fg = fg_cell, .bg = bg, .flags = current_flags };
    }

    const last = StyleRun{
        .start_col = run_start,
        .end_col = @intCast(cells.len),
        .fg_r = current_resolved.fg.r,
        .fg_g = current_resolved.fg.g,
        .fg_b = current_resolved.fg.b,
        .bg_r = current_resolved.bg.r,
        .bg_g = current_resolved.bg.g,
        .bg_b = current_resolved.bg.b,
        .flags = current_resolved.flags,
        .reserved = 0,
    };
    out.appendSlice(std.mem.asBytes(&last)) catch return .{ .ptr = null, .len = 0 };

    const slice = out.toOwnedSlice() catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = slice.ptr, .len = slice.len };
}

export fn ghostty_vt_terminal_take_dirty_viewport_rows(
    terminal_ptr: ?*anyopaque,
    rows: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null or rows == 0) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const alloc = std.heap.c_allocator;

    var out = std.ArrayList(u8).init(alloc);
    errdefer out.deinit();

    const force_full_redraw = handle.terminal.flags.dirty.clear;
    if (force_full_redraw) {
        handle.terminal.flags.dirty.clear = false;
    }

    var y: u32 = 0;
    while (y < rows) : (y += 1) {
        const pt: terminal.point.Point = .{ .viewport = .{ .x = 0, .y = y } };
        const pin = handle.terminal.screen.pages.pin(pt) orelse continue;
        if (!force_full_redraw and !pin.isDirty()) continue;

        const v: u16 = @intCast(y);
        out.append(@intCast(v & 0xFF)) catch return .{ .ptr = null, .len = 0 };
        out.append(@intCast((v >> 8) & 0xFF)) catch return .{ .ptr = null, .len = 0 };

        var set = pin.node.data.dirtyBitSet();
        set.unset(@intCast(pin.y));
    }

    const slice = out.toOwnedSlice() catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = slice.ptr, .len = slice.len };
}

export fn ghostty_vt_terminal_hyperlink_at(
    terminal_ptr: ?*anyopaque,
    col: u16,
    row: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (terminal_ptr == null or col == 0 or row == 0) return .{ .ptr = null, .len = 0 };
    const handle: *TerminalHandle = @ptrCast(@alignCast(terminal_ptr.?));

    const x: terminal.size.CellCountInt = @intCast(col - 1);
    const y: u32 = @intCast(row - 1);
    const pt: terminal.point.Point = .{ .viewport = .{ .x = x, .y = y } };
    const pin = handle.terminal.screen.pages.pin(pt) orelse return .{ .ptr = null, .len = 0 };
    const rac = pin.rowAndCell();
    if (!rac.cell.hyperlink) return .{ .ptr = null, .len = 0 };

    const id = pin.node.data.lookupHyperlink(rac.cell) orelse return .{ .ptr = null, .len = 0 };
    const entry = pin.node.data.hyperlink_set.get(pin.node.data.memory, id).*;
    const uri = entry.uri.offset.ptr(pin.node.data.memory)[0..entry.uri.len];

    const alloc = std.heap.c_allocator;
    const duped = alloc.dupe(u8, uri) catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = duped.ptr, .len = duped.len };
}

export fn ghostty_vt_encode_key_named(
    name_ptr: ?[*]const u8,
    name_len: usize,
    modifiers: u16,
) callconv(.C) ghostty_vt_bytes_t {
    if (name_ptr == null or name_len == 0) return .{ .ptr = null, .len = 0 };

    const name = name_ptr.?[0..name_len];

    const key_value: ghostty_input.Key = if (std.mem.eql(u8, name, "up"))
        .arrow_up
    else if (std.mem.eql(u8, name, "down"))
        .arrow_down
    else if (std.mem.eql(u8, name, "left"))
        .arrow_left
    else if (std.mem.eql(u8, name, "right"))
        .arrow_right
    else if (std.mem.eql(u8, name, "home"))
        .home
    else if (std.mem.eql(u8, name, "end"))
        .end
    else if (std.mem.eql(u8, name, "pageup") or std.mem.eql(u8, name, "page_up") or std.mem.eql(u8, name, "page-up"))
        .page_up
    else if (std.mem.eql(u8, name, "pagedown") or std.mem.eql(u8, name, "page_down") or std.mem.eql(u8, name, "page-down"))
        .page_down
    else if (std.mem.eql(u8, name, "insert"))
        .insert
    else if (std.mem.eql(u8, name, "delete"))
        .delete
    else if (std.mem.eql(u8, name, "backspace"))
        .backspace
    else if (std.mem.eql(u8, name, "enter"))
        .enter
    else if (std.mem.eql(u8, name, "tab"))
        .tab
    else if (std.mem.eql(u8, name, "escape"))
        .escape
    else if (name.len >= 2 and name[0] == 'f')
        parse_function_key(name[1..]) orelse return .{ .ptr = null, .len = 0 }
    else
        return .{ .ptr = null, .len = 0 };

    var mods: ghostty_input.Mods = .{};
    if ((modifiers & 0x0001) != 0) mods.shift = true;
    if ((modifiers & 0x0002) != 0) mods.ctrl = true;
    if ((modifiers & 0x0004) != 0) mods.alt = true;
    if ((modifiers & 0x0008) != 0) mods.super = true;

    const event: ghostty_input.KeyEvent = .{
        .action = .press,
        .key = key_value,
        .mods = mods,
    };

    const enc: ghostty_input.KeyEncoder = .{
        .event = event,
        .alt_esc_prefix = true,
    };

    var buf: [128]u8 = undefined;
    const encoded = enc.encode(buf[0..]) catch return .{ .ptr = null, .len = 0 };
    if (encoded.len == 0) return .{ .ptr = null, .len = 0 };

    const alloc = std.heap.c_allocator;
    const duped = alloc.dupe(u8, encoded) catch return .{ .ptr = null, .len = 0 };
    return .{ .ptr = duped.ptr, .len = duped.len };
}

fn parse_function_key(digits: []const u8) ?ghostty_input.Key {
    if (digits.len == 1) {
        return switch (digits[0]) {
            '1' => .f1,
            '2' => .f2,
            '3' => .f3,
            '4' => .f4,
            '5' => .f5,
            '6' => .f6,
            '7' => .f7,
            '8' => .f8,
            '9' => .f9,
            else => null,
        };
    }

    if (digits.len == 2 and digits[0] == '1') {
        return switch (digits[1]) {
            '0' => .f10,
            '1' => .f11,
            '2' => .f12,
            else => null,
        };
    }

    return null;
}

const ghostty_vt_bytes_t = extern struct {
    ptr: ?[*]const u8,
    len: usize,
};

export fn ghostty_vt_bytes_free(bytes: ghostty_vt_bytes_t) callconv(.C) void {
    if (bytes.ptr == null or bytes.len == 0) return;
    std.heap.c_allocator.free(bytes.ptr.?[0..bytes.len]);
}

// Ghostty's terminal stream uses this symbol as an optimization hook.
// Provide a portable scalar implementation so we don't need C++ SIMD deps.
export fn ghostty_simd_decode_utf8_until_control_seq(
    input: [*]const u8,
    count: usize,
    output: [*]u32,
    output_count: *usize,
) callconv(.C) usize {
    var i: usize = 0;
    var out_i: usize = 0;
    while (i < count) {
        if (input[i] == 0x1B) break;

        const b0 = input[i];
        var cp: u32 = 0xFFFD;
        var need: usize = 1;

        if (b0 < 0x80) {
            cp = b0;
            need = 1;
        } else if (b0 & 0xE0 == 0xC0) {
            need = 2;
            if (i + need > count) break;
            const b1 = input[i + 1];
            if (b1 & 0xC0 != 0x80) {
                cp = 0xFFFD;
                need = 1;
            } else {
                cp = ((@as(u32, b0 & 0x1F)) << 6) | (@as(u32, b1 & 0x3F));
            }
        } else if (b0 & 0xF0 == 0xE0) {
            need = 3;
            if (i + need > count) break;
            const b1 = input[i + 1];
            const b2 = input[i + 2];
            if (b1 & 0xC0 != 0x80 or b2 & 0xC0 != 0x80) {
                cp = 0xFFFD;
                need = 1;
            } else {
                cp = ((@as(u32, b0 & 0x0F)) << 12) |
                    ((@as(u32, b1 & 0x3F)) << 6) |
                    (@as(u32, b2 & 0x3F));
            }
        } else if (b0 & 0xF8 == 0xF0) {
            need = 4;
            if (i + need > count) break;
            const b1 = input[i + 1];
            const b2 = input[i + 2];
            const b3 = input[i + 3];
            if (b1 & 0xC0 != 0x80 or b2 & 0xC0 != 0x80 or b3 & 0xC0 != 0x80) {
                cp = 0xFFFD;
                need = 1;
            } else {
                cp = ((@as(u32, b0 & 0x07)) << 18) |
                    ((@as(u32, b1 & 0x3F)) << 12) |
                    ((@as(u32, b2 & 0x3F)) << 6) |
                    (@as(u32, b3 & 0x3F));
            }
        } else {
            cp = 0xFFFD;
            need = 1;
        }

        output[out_i] = cp;
        out_i += 1;
        i += need;
    }

    output_count.* = out_i;
    return i;
}
