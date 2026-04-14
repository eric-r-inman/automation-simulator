module Widget.YardMap exposing (view)

{-| Schematic SVG yard map.

The simulator does not (yet) carry per-zone polygon coordinates,
so the map auto-lays out zones into a grid keyed by their yard.
Zones in the same yard share a column; zones across yards stack
side-by-side. Each zone's fill color tracks its current soil
moisture: deep green at field capacity, fading through yellow and
russet to dark brown as VWC drops toward wilting point. An open
valve gets a contrasting blue stroke so the user can see at a
glance which zones are currently watering.

Click a zone to focus its card; the dashboard wires this through
the optional `onClickZone` argument.

-}

import Api
import Dict exposing (Dict)
import Html exposing (Html)
import Svg exposing (Svg)
import Svg.Attributes as SAttr
import Svg.Events



-- ── View ──────────────────────────────────────────────────────────────────


type alias Args msg =
    { property : Api.Property
    , state : Api.SimState
    , onClickZone : String -> msg
    , focusedZoneId : Maybe String
    }


view : Args msg -> Html msg
view args =
    let
        zonesByYard : Dict String (List Api.Zone)
        zonesByYard =
            args.property.zones
                |> List.foldl
                    (\z acc ->
                        Dict.update z.yardId
                            (\existing ->
                                case existing of
                                    Just list ->
                                        Just (list ++ [ z ])

                                    Nothing ->
                                        Just [ z ]
                            )
                            acc
                    )
                    Dict.empty

        stateByZone : Dict String Api.ZoneState
        stateByZone =
            args.state.zones
                |> List.map (\z -> ( z.zoneId, z ))
                |> Dict.fromList

        yardWidth : Int
        yardWidth =
            240

        yardSpacing : Int
        yardSpacing =
            40

        yardCount : Int
        yardCount =
            List.length args.property.yards

        totalWidth : Int
        totalWidth =
            yardCount * (yardWidth + yardSpacing) + yardSpacing

        totalHeight : Int
        totalHeight =
            520

        viewBox : String
        viewBox =
            "0 0 "
                ++ String.fromInt totalWidth
                ++ " "
                ++ String.fromInt totalHeight

        yardSvgs : List (Svg msg)
        yardSvgs =
            args.property.yards
                |> List.indexedMap
                    (\index yard ->
                        let
                            x : Int
                            x =
                                yardSpacing + index * (yardWidth + yardSpacing)

                            zonesHere : List Api.Zone
                            zonesHere =
                                Dict.get yard.id zonesByYard
                                    |> Maybe.withDefault []
                        in
                        yardGroup
                            { x = x
                            , width = yardWidth
                            , height = totalHeight - 80
                            , yard = yard
                            , zones = zonesHere
                            , stateByZone = stateByZone
                            , focusedZoneId = args.focusedZoneId
                            , onClickZone = args.onClickZone
                            }
                    )
    in
    Svg.svg
        [ SAttr.viewBox viewBox
        , SAttr.preserveAspectRatio "xMidYMid meet"
        , SAttr.class "yard-map"
        ]
        yardSvgs



-- ── Yard group ────────────────────────────────────────────────────────────


type alias YardGroupArgs msg =
    { x : Int
    , width : Int
    , height : Int
    , yard : Api.Yard
    , zones : List Api.Zone
    , stateByZone : Dict String Api.ZoneState
    , focusedZoneId : Maybe String
    , onClickZone : String -> msg
    }


yardGroup : YardGroupArgs msg -> Svg msg
yardGroup args =
    let
        yardLabelY : Int
        yardLabelY =
            30

        zoneAreaTopY : Int
        zoneAreaTopY =
            50

        zoneCount : Int
        zoneCount =
            List.length args.zones

        zoneHeight : Int
        zoneHeight =
            if zoneCount == 0 then
                args.height - 60

            else
                (args.height - 80) // zoneCount

        zoneSvgs : List (Svg msg)
        zoneSvgs =
            args.zones
                |> List.indexedMap
                    (\i zone ->
                        let
                            y : Int
                            y =
                                zoneAreaTopY + i * zoneHeight

                            zState : Maybe Api.ZoneState
                            zState =
                                Dict.get zone.id args.stateByZone

                            isFocused : Bool
                            isFocused =
                                args.focusedZoneId == Just zone.id
                        in
                        zoneRect
                            { x = args.x + 12
                            , y = y
                            , width = args.width - 24
                            , height = zoneHeight - 12
                            , zone = zone
                            , state = zState
                            , isFocused = isFocused
                            , onClick = args.onClickZone zone.id
                            }
                    )
    in
    Svg.g []
        (Svg.rect
            [ SAttr.x (String.fromInt args.x)
            , SAttr.y "10"
            , SAttr.width (String.fromInt args.width)
            , SAttr.height (String.fromInt args.height)
            , SAttr.rx "12"
            , SAttr.fill "#e8e3d0"
            , SAttr.stroke "#8a7e5e"
            , SAttr.strokeWidth "2"
            ]
            []
            :: Svg.text_
                [ SAttr.x (String.fromInt (args.x + args.width // 2))
                , SAttr.y (String.fromInt yardLabelY)
                , SAttr.textAnchor "middle"
                , SAttr.class "yard-label"
                ]
                [ Svg.text args.yard.name ]
            :: zoneSvgs
        )



-- ── Zone rectangle ────────────────────────────────────────────────────────


type alias ZoneRectArgs msg =
    { x : Int
    , y : Int
    , width : Int
    , height : Int
    , zone : Api.Zone
    , state : Maybe Api.ZoneState
    , isFocused : Bool
    , onClick : msg
    }


zoneRect : ZoneRectArgs msg -> Svg msg
zoneRect args =
    let
        vwc : Float
        vwc =
            args.state |> Maybe.map .soilVwc |> Maybe.withDefault 0.0

        valveOpen : Bool
        valveOpen =
            args.state |> Maybe.map .valveIsOpen |> Maybe.withDefault False

        fill : String
        fill =
            moistureColor vwc

        stroke : String
        stroke =
            if valveOpen then
                "#2266bb"

            else if args.isFocused then
                "#8a7e5e"

            else
                "#5a5040"

        strokeWidth : String
        strokeWidth =
            if valveOpen || args.isFocused then
                "3"

            else
                "1"

        labelY : Int
        labelY =
            args.y + args.height // 2 - 4

        valueY : Int
        valueY =
            args.y + args.height // 2 + 16

        plantKindLabel : String
        plantKindLabel =
            displayPlantKind args.zone.plantKind

        moistureLabel : String
        moistureLabel =
            String.fromInt (round (vwc * 100)) ++ "% VWC"
    in
    Svg.g
        [ Svg.Events.onClick args.onClick
        , SAttr.cursor "pointer"
        ]
        [ Svg.rect
            [ SAttr.x (String.fromInt args.x)
            , SAttr.y (String.fromInt args.y)
            , SAttr.width (String.fromInt args.width)
            , SAttr.height (String.fromInt args.height)
            , SAttr.rx "8"
            , SAttr.fill fill
            , SAttr.stroke stroke
            , SAttr.strokeWidth strokeWidth
            ]
            []
        , Svg.text_
            [ SAttr.x (String.fromInt (args.x + args.width // 2))
            , SAttr.y (String.fromInt labelY)
            , SAttr.textAnchor "middle"
            , SAttr.class "zone-name"
            ]
            [ Svg.text args.zone.id ]
        , Svg.text_
            [ SAttr.x (String.fromInt (args.x + args.width // 2))
            , SAttr.y (String.fromInt valueY)
            , SAttr.textAnchor "middle"
            , SAttr.class "zone-value"
            ]
            [ Svg.text (plantKindLabel ++ " · " ++ moistureLabel) ]
        ]



-- ── Color scale ───────────────────────────────────────────────────────────


{-| Map VWC to a hex color. Stops chosen so a healthy zone (~0.30)
sits in the deep-green band and a stressed zone (~0.15) reads
russet — matches the earthy palette in style.css.
-}
moistureColor : Float -> String
moistureColor vwc =
    if vwc >= 0.4 then
        "#2f5d34"

    else if vwc >= 0.32 then
        "#3f7a3f"

    else if vwc >= 0.25 then
        "#7aa84a"

    else if vwc >= 0.18 then
        "#c4a04a"

    else if vwc >= 0.12 then
        "#a55d2a"

    else
        "#5a3a20"


displayPlantKind : String -> String
displayPlantKind raw =
    case raw of
        "veggiebed" ->
            "veggie"

        "veggie-bed" ->
            "veggie"

        "shrub" ->
            "shrub"

        "perennial" ->
            "perennial"

        "tree" ->
            "tree"

        other ->
            other
