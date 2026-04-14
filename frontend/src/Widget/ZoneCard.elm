module Widget.ZoneCard exposing (view)

{-| Per-zone card. Shows current moisture, valve state, total open
seconds, and exposes Run / Stop buttons that produce messages the
dashboard turns into POST requests.
-}

import Api
import Html exposing (Html, button, dd, div, dl, dt, h3, span, text)
import Html.Attributes as Attr
import Html.Events exposing (onClick)


type alias Args msg =
    { zone : Api.Zone
    , state : Maybe Api.ZoneState
    , status : Maybe Api.ZoneStatus
    , onRun : Int -> msg
    , onStop : msg
    , onFocus : msg
    , onEdit : msg
    , onDelete : msg
    , isFocused : Bool
    }


view : Args msg -> Html msg
view args =
    let
        vwc : Float
        vwc =
            args.state |> Maybe.map .soilVwc |> Maybe.withDefault 0.0

        valveOpen : Bool
        valveOpen =
            args.state |> Maybe.map .valveIsOpen |> Maybe.withDefault False

        totalOpenSeconds : Int
        totalOpenSeconds =
            args.status
                |> Maybe.map .totalOpenSeconds
                |> Maybe.withDefault 0

        moistureLabel : String
        moistureLabel =
            String.fromFloat (toFixed 2 vwc) ++ " v/v"

        valveBadge : Html msg
        valveBadge =
            if valveOpen then
                span [ Attr.class "badge badge-open" ] [ text "OPEN" ]

            else
                span [ Attr.class "badge badge-closed" ] [ text "closed" ]

        focusedClass : String
        focusedClass =
            if args.isFocused then
                "zone-card zone-card-focused"

            else
                "zone-card"
    in
    div
        [ Attr.class focusedClass
        , onClick args.onFocus
        ]
        [ div [ Attr.class "zone-card-header" ]
            [ h3 [] [ text args.zone.id ]
            , valveBadge
            ]
        , dl [ Attr.class "zone-stats" ]
            [ dt [] [ text "Plant" ]
            , dd [] [ text args.zone.plantKind ]
            , dt [] [ text "Area" ]
            , dd []
                [ text (String.fromFloat args.zone.areaSqFt ++ " sq ft") ]
            , dt [] [ text "Moisture" ]
            , dd [] [ text moistureLabel ]
            , dt [] [ text "Total open" ]
            , dd []
                [ text
                    (formatDuration totalOpenSeconds)
                ]
            ]
        , div [ Attr.class "zone-actions" ]
            [ button
                [ Attr.class "btn btn-run"
                , onClick (args.onRun 5)
                ]
                [ text "Run 5 min" ]
            , button
                [ Attr.class "btn btn-run"
                , onClick (args.onRun 15)
                ]
                [ text "Run 15 min" ]
            , button
                [ Attr.class "btn btn-stop"
                , onClick args.onStop
                ]
                [ text "Stop" ]
            ]
        , div [ Attr.class "zone-meta-actions" ]
            [ button
                [ Attr.class "btn btn-edit"
                , onClick args.onEdit
                ]
                [ text "Edit" ]
            , button
                [ Attr.class "btn btn-delete"
                , onClick args.onDelete
                ]
                [ text "Delete" ]
            ]
        ]


formatDuration : Int -> String
formatDuration seconds =
    if seconds < 60 then
        String.fromInt seconds ++ " s"

    else if seconds < 3600 then
        String.fromInt (seconds // 60) ++ " min"

    else
        String.fromInt (seconds // 3600) ++ " h"


toFixed : Int -> Float -> Float
toFixed places value =
    let
        scale : Float
        scale =
            10.0 ^ toFloat places
    in
    toFloat (round (value * scale)) / scale
