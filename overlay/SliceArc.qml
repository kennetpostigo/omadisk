import QtQuick
import QtQuick.Shapes
import qs.Commons

Item {
  id: root

  property var overlay: null
  property int sliceIndex: 0
  property real cx: 0
  property real cy: 0

  readonly property var live: {
    if (overlay) void overlay.hoverTick
    return overlay && overlay.sliceModel ? overlay.sliceModel.get(sliceIndex) : null
  }
  readonly property string slicePath: live && live.path ? String(live.path) : ""
  readonly property string sliceKind: live && live.kind ? String(live.kind) : ""
  readonly property int sliceRing: live ? Number(live.ring) || 0 : 0
  readonly property real startDeg: live ? Number(live.startDeg) || 0 : 0
  readonly property real sweepDeg: live ? Number(live.sweepDeg) || 0 : 0
  readonly property real innerR: live ? Number(live.innerR) || 0 : 0
  readonly property real outerR: live ? Number(live.outerR) || 0 : 0
  readonly property color sliceColor: live && live.color ? String(live.color) : "#2ee36a"

  readonly property bool hovered: {
    if (!overlay || !slicePath) return false
    void overlay.hoverTick
    if (slicePath === overlay.hoverPath) return true
    if (sliceKind === "other" && sliceRing === 1)
      return overlay.hoverIsListRow && !overlay.hoverHasSlice
    return false
  }

  readonly property real midR: (innerR + outerR) / 2
  readonly property real thickness: Math.max(0, outerR - innerR)

  Shape {
    anchors.fill: parent
    preferredRendererType: Shape.CurveRenderer
    opacity: overlay && overlay.hoverPath !== "" && !root.hovered ? 0.55 : 1

    ShapePath {
      strokeWidth: root.thickness
      strokeColor: root.sliceColor
      fillColor: "transparent"
      capStyle: ShapePath.FlatCap
      PathAngleArc {
        centerX: root.cx
        centerY: root.cy
        radiusX: root.midR
        radiusY: root.midR
        startAngle: root.startDeg
        sweepAngle: root.sweepDeg
      }
    }

    ShapePath {
      strokeWidth: root.thickness + 3
      strokeColor: root.hovered ? Color.accent : "transparent"
      fillColor: "transparent"
      capStyle: ShapePath.FlatCap
      PathAngleArc {
        centerX: root.cx
        centerY: root.cy
        radiusX: root.midR
        radiusY: root.midR
        startAngle: root.startDeg
        sweepAngle: root.sweepDeg
      }
    }
  }
}
