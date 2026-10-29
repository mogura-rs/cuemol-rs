import cuemol as cm


def setupDefaultRenderer(obj):
    scene = obj.getScene()
    with cm.UndoTxn("Create default renderer", scene):
        renderer = obj.createRenderer("simple")
        renderer.applyStyles("DefaultCPKColoring")
        renderer.name = "simple1"
        renderer.sel = cm.sel("*", scene)
        view = cm.view(scene)
        if view is not None:
            view.setViewCenter(renderer.getCenter())
    return renderer


def create_default_paint_coloring():
    coloring = cm.createObj("PaintColoring")
    for selection, color in (("sheet", "SteelBlue"), ("helix", "khaki"),
                             ("nucleic", "yellow"), ("*", "FloralWhite")):
        coloring.append(cm.sel(selection), cm.col(color))
    return coloring


def set_default_styles(rend):
    styles = {
        "tube": "DefaultHSCPaint", "spline": "DefaultHSCPaint",
        "ribbon": "DefaultRibbon,DefaultHSCPaint",
        "cartoon": "DefaultCartoon,DefaultHSCPaint", "nucl": "DefaultNucl",
        "anisou": "DefaultAnIsoU,DefaultCPKColoring",
        "ballstick": "DefaultBallStick,DefaultCPKColoring",
        "cpk": "DefaultCPK,DefaultCPKColoring", "contour": "DefaultContour",
        "isosurf": "DefaultIsoSurf",
    }
    style = styles.get(rend.type_name)
    if style is None and "coloring" in rend:
        style = "DefaultCPKColoring"
    if style is not None:
        rend.applyStyles(style)


def create_renderer(obj, rend_type, rend_name=None):
    renderer = obj.createRenderer(rend_type)
    if rend_name is not None:
        renderer.name = rend_name
    obj.coloring = create_default_paint_coloring()
    set_default_styles(renderer)
    return renderer
