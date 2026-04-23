module Widget.ZoneCard exposing (view)

{-| Per-zone card. Shows current moisture, valve state, total open
seconds, and exposes Run / Stop buttons that produce messages the
dashboard turns into POST requests.
-}

import Api
import Html exposing (Html, button, dd, div, dl, dt, h3, input, span, text)
import Html.Attributes as Attr
import Html.Events exposing (onClick, onInput)


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
    , customMinutes : String
    , onSetCustomMinutes : String -> msg
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

        parsedCustomMinutes : Maybe Int
        parsedCustomMinutes =
            String.toInt (String.trim args.customMinutes)
                |> Maybe.andThen
                    (\n ->
                        if n > 0 then
                            Just n

                        else
                            Nothing
                    )

        customRunDisabled : Bool
        customRunDisabled =
            parsedCustomMinutes == Nothing

        customRunClick : List (Html.Attribute msg)
        customRunClick =
            case parsedCustomMinutes of
                Just n ->
                    [ onClick (args.onRun n) ]

                Nothing ->
                    []
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
            [ runPresetButton args 5 "5 min"
            , runPresetButton args 15 "15 min"
            , runPresetButton args 30 "30 min"
            , runPresetButton args 60 "1 h"
            , runPresetButton args 120 "2 h"
            , button
                [ Attr.class "btn btn-stop"
                , onClick args.onStop
                ]
                [ text "Stop" ]
            ]
        , div [ Attr.class "zone-custom-run" ]
            [ input
                [ Attr.type_ "number"
                , Attr.min "1"
                , Attr.placeholder "minutes"
                , Attr.value args.customMinutes
                , Attr.class "custom-minutes"
                , onInput args.onSetCustomMinutes
                ]
                []
            , button
                (Attr.class "btn btn-run"
                    :: Attr.disabled customRunDisabled
                    :: customRunClick
                )
                [ text "Run" ]
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


runPresetButton : Args msg -> Int -> String -> Html msg
runPresetButton args minutes label_ =
    button
        [ Attr.class "btn btn-run"
        , onClick (args.onRun minutes)
        ]
        [ text label_ ]


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
