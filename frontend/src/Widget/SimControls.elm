module Widget.SimControls exposing (view)

{-| Step / Reset controls for the simulation clock. Lives at the
top of the dashboard so it stays in view as the page scrolls.
-}

import Html exposing (Html, button, div, h3, p, span, text)
import Html.Attributes as Attr
import Html.Events exposing (onClick)


type alias Args msg =
    { simulatedMinutes : Int
    , simulatedDatetime : String
    , onStep : Int -> msg
    , onReset : msg
    , isBusy : Bool
    }


view : Args msg -> Html msg
view args =
    div [ Attr.class "sim-controls" ]
        [ div [ Attr.class "sim-controls-header" ]
            [ h3 [] [ text "Simulator" ]
            , div [ Attr.class "sim-clock" ]
                [ span [ Attr.class "sim-clock-datetime" ]
                    [ text args.simulatedDatetime ]
                , span [ Attr.class "sim-clock-elapsed" ]
                    [ text (humanizeMinutes args.simulatedMinutes ++ " elapsed") ]
                ]
            ]
        , div [ Attr.class "sim-step-buttons" ]
            [ stepButton args 60 "+1 hour"
            , stepButton args (60 * 6) "+6 hours"
            , stepButton args (60 * 24) "+1 day"
            , stepButton args (60 * 24 * 7) "+1 week"
            , button
                [ Attr.class "btn btn-reset"
                , Attr.disabled args.isBusy
                , onClick args.onReset
                ]
                [ text "Reset" ]
            ]
        , p [ Attr.class "sim-hint" ]
            [ text "Stepping advances the simulated clock and runs the engine; the dashboard re-fetches state on every step." ]
        ]


stepButton : Args msg -> Int -> String -> Html msg
stepButton args minutes label =
    button
        [ Attr.class "btn btn-step"
        , Attr.disabled args.isBusy
        , onClick (args.onStep minutes)
        ]
        [ text label ]


humanizeMinutes : Int -> String
humanizeMinutes m =
    if m < 60 then
        String.fromInt m ++ " min"

    else if m < 60 * 24 then
        String.fromInt (m // 60) ++ " h"

    else if m < 60 * 24 * 7 then
        String.fromInt (m // (60 * 24)) ++ " d"

    else
        String.fromInt (m // (60 * 24 * 7)) ++ " w"
