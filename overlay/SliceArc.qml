import QtQuick
import QtQuick.Shapes
import qs.Commons

Item {
  id: root

  property var slice: ({})
  property var overlay: null
  property real cx: 0
  property real cy: 0

  readonly property bool hovered: {
    if (!overlay || !slice || !slice.path) return false
    if (slice.path === overlay.hoverPath) return true
    var ring1Other = overlay.currentView
      ? String(overlay.currentView.path || "") + "/\0other"
      : ""
    if (slice.kind === "other" && slice.ring === 1 && slice.path === ring1Other) {
      if (overlay.hoverPath === "" || overlay.hoverPath === overlay.focusPath)
        return false
      return overlay.hoverIsListRow && !overlay.hoverHasSlice
    }
    return false
  }

  readonly property real midR: ((slice.innerR || 0) + (slice.outerR || 0)) / 2
  readonly property real thickness: (slice.outerR || 0) - (slice.innerR || 0)

  Shape {
    anchors.fill: parent
    preferredRendererType: Shape.CurveRenderer
    opacity: overlay && overlay.hoverPath !== "" && !root.hovered ? 0.42 : 1

    ShapePath {
      strokeWidth: root.thickness
      strokeColor: slice.color || "#3daf6b"
      fillColor: "transparent"
      capStyle: ShapePath.FlatCap
      PathAngleArc {
        centerX: root.cx
        centerY: root.cy
        radiusX: root.midR
        radiusY: root.midR
        startAngle: slice.startDeg || 0
        sweepAngle: slice.sweepDeg || 0
      }
    }

    ShapePath {
      strokeWidth: root.thickness + 2
      strokeColor: root.hovered ? Color.accent : "transparent"
      fillColor: "transparent"
      capStyle: ShapePath.FlatCap
      PathAngleArc {
        centerX: root.cx
        centerY: root.cy
        radiusX: root.midR
        radiusY: root.midR
        startAngle: slice.startDeg || 0
        sweepAngle: slice.sweepDeg || 0
      }
    }
  }
}
