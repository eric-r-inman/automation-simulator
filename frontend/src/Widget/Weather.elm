module Widget.Weather exposing (view)

{-| Weather panel. Tiny snapshot of the latest sample with units
spelled out, plus a banner badge when there's measurable rain so
the user notices it even from across the room.
-}

import Api
import Html exposing (Html, div, h3, p, span, text)
import Html.Attributes as Attr


view : Api.Weather -> Html msg
view w =
    let
        rainBadge : Html msg
        rainBadge =
            if w.precipitationMmPerHour > 0.05 then
                span [ Attr.class "badge badge-rain" ]
                    [ text "raining" ]

            else
                text ""
    in
    div [ Attr.class "weather-panel" ]
        [ div [ Attr.class "weather-header" ]
            [ h3 [] [ text "Weather now" ]
            , rainBadge
            ]
        , div [ Attr.class "weather-grid" ]
            [ stat "Temp"
                (String.fromFloat (toFixed 1 w.temperatureC) ++ " °C")
            , stat "Humidity"
                (String.fromFloat (toFixed 0 w.humidityPct) ++ " %")
            , stat "Wind"
                (String.fromFloat (toFixed 1 w.windMPerS) ++ " m/s")
            , stat "Solar"
                (String.fromFloat (toFixed 0 w.solarWPerM2) ++ " W/m²")
            , stat "Rain"
                (String.fromFloat (toFixed 2 w.precipitationMmPerHour)
                    ++ " mm/h"
                )
            ]
        , p [ Attr.class "weather-note" ]
            [ text "Sourced from the simulator's Portland-OR climatology + seeded rain events." ]
        ]


stat : String -> String -> Html msg
stat label value =
    div [ Attr.class "weather-stat" ]
        [ div [ Attr.class "weather-stat-label" ] [ text label ]
        , div [ Attr.class "weather-stat-value" ] [ text value ]
        ]


toFixed : Int -> Float -> Float
toFixed places value =
    let
        scale : Float
        scale =
            10.0 ^ toFloat places
    in
    toFloat (round (value * scale)) / scale
