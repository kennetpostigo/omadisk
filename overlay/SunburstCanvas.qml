import QtQuick
import qs.Commons
import "OverlayModel.js" as Model
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text

  readonly property real size: Math.min(width, height)
  readonly property real cx: width / 2
  readonly property real cy: height / 2
  readonly property real hubR: overlay && overlay.slices && overlay.slices.length
    ? Number(overlay.slices[0].innerR) || (0.34 * size / 2)
    : 0.34 * size / 2

  function paint() { canvas.requestPaint() }

  onWidthChanged: paint()
  onHeightChanged: paint()

  Connections {
    target: overlay
    function onHoverTickChanged() { canvas.requestPaint() }
    function onHoverPathChanged() { canvas.requestPaint() }
    function onSlicesChanged() { canvas.requestPaint() }
    function onFocusPathChanged() { canvas.requestPaint() }
  }

  Canvas {
    id: canvas
    anchors.fill: parent
    renderTarget: Canvas.FramebufferObject
    antialiasing: true
    onPaint: {
      var ctx = getContext("2d")
      var w = width
      var h = height
      ctx.reset()
      ctx.clearRect(0, 0, w, h)
      if (!overlay) return
      var slices = overlay.slices || []
      var hover = overlay.hoverPath || ""
      var i
      // A list row that was collapsed into Other does not own that wedge.
      // Highlight only the slice whose path matches hoverPath.
      function isActive(slice) {
        return !!hover && slice.path === hover
      }
      for (i = 0; i < slices.length; i++)
        drawSlice(ctx, slices[i], isActive(slices[i]))
      if (overlay.hoverHasSlice) {
        for (i = 0; i < slices.length; i++) {
          if (isActive(slices[i]))
            drawSlice(ctx, slices[i], true)
        }
      }
    }
  }

  function drawSlice(ctx, slice, active) {
    var a0 = (Number(slice.startDeg) || 0) * Math.PI / 180
    var a1 = a0 + (Number(slice.sweepDeg) || 0) * Math.PI / 180
    if (a1 <= a0) return
    var inner = Number(slice.innerR) || 0
    var outer = Number(slice.outerR) || 0
    if (outer <= inner + 0.5) return
    ctx.beginPath()
    ctx.arc(cx, cy, outer, a0, a1, false)
    ctx.arc(cx, cy, inner, a1, a0, true)
    ctx.closePath()
    ctx.fillStyle = active ? Model.mixHex(slice.fill || "#64748B", "#ffffff", 0.22) : (slice.fill || "#64748B")
    ctx.globalAlpha = !overlay.hoverHasSlice || active ? 1 : 0.42
    ctx.fill()
    ctx.globalAlpha = 1
    if (active) {
      ctx.strokeStyle = "#ffffff"
      ctx.lineWidth = 1.5
      ctx.stroke()
    }
  }

  HubLabel {
    width: root.hubR * 2
    height: root.hubR * 2
    anchors.centerIn: parent
    overlay: root.overlay
    foreground: root.foreground
  }

  EmptyState {
    anchors.fill: parent
    overlay: root.overlay
    foreground: root.foreground
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    preventStealing: true
    enabled: !(overlay && overlay.offerHome && (!overlay.slices || overlay.slices.length === 0))
    cursorShape: Qt.ArrowCursor
    onPositionChanged: function(mouse) {
      if (!overlay) return
      var hit = Model.hitTestSlices(mouse.x, mouse.y, root.cx, root.cy, overlay.slices, root.hubR)
      if (hit.kind === "slice") {
        overlay.syncSelection(hit.slice.path)
        cursorShape = hit.slice.kind === "dir" ? Qt.PointingHandCursor : Qt.ArrowCursor
      } else if (hit.kind === "hub") {
        overlay.syncSelection(overlay.focusPath)
        cursorShape = Qt.PointingHandCursor
      } else {
        cursorShape = Qt.ArrowCursor
      }
    }
    onPressed: function(mouse) {
      if (!overlay) return
      if (overlay.offerHome) {
        overlay.fallBackHome()
        return
      }
      var hit = Model.hitTestSlices(mouse.x, mouse.y, root.cx, root.cy, overlay.slices, root.hubR)
      if (hit.kind === "hub") {
        overlay.goUp()
        return
      }
      if (hit.kind === "slice")
        overlay.activatePath(hit.slice.path)
    }
  }
}
