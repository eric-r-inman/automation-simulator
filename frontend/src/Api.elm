module Api exposing
    ( Property
    , Reading
    , ResetResult
    , RunResult
    , SimState
    , Spigot
    , StepResult
    , StopResult
    , Weather
    , Yard
    , Zone
    , ZoneState
    , ZoneStatus
    , ZonesResponse
    , fetchProperty
    , fetchSensors
    , fetchState
    , fetchWeather
    , fetchZones
    , postReset
    , postRunZone
    , postStep
    , postStopZone
    )

{-| HTTP wiring against the Phase 7a sim routes.

Each endpoint exposes one `Cmd Msg`-producing function plus the
record types its response decodes into. Decoders fail loudly on a
shape mismatch so the dashboard can render a typed `Failed` state
rather than rendering with stale or empty data.

-}

import Http
import Json.Decode as Decode exposing (Decoder)
import Json.Encode as Encode



-- ── Types ─────────────────────────────────────────────────────────────────


type alias Property =
    { id : String
    , name : String
    , climateZone : String
    , lotAreaSqFt : Float
    , yards : List Yard
    , spigots : List Spigot
    , zones : List Zone
    }


type alias Yard =
    { id : String
    , name : String
    , areaSqFt : Float
    }


type alias Spigot =
    { id : String
    , mainsPressurePsi : Float
    , notes : Maybe String
    }


type alias Zone =
    { id : String
    , yardId : String
    , manifoldId : String
    , plantKind : String
    , areaSqFt : Float
    }


type alias SimState =
    { simulatedMinutesElapsed : Int
    , simulatedDatetime : String
    , zones : List ZoneState
    , weather : Weather
    }


type alias ZoneState =
    { zoneId : String
    , soilVwc : Float
    , valveIsOpen : Bool
    }


type alias Weather =
    { temperatureC : Float
    , humidityPct : Float
    , windMPerS : Float
    , solarWPerM2 : Float
    , precipitationMmPerHour : Float
    }


type alias StepResult =
    { minutesAdvanced : Int
    , simulatedMinutesElapsed : Int
    }


type alias ResetResult =
    { resetAt : String
    }


type alias ZoneStatus =
    { zoneId : String
    , isOpen : Bool
    , openUntilMinutes : Maybe Int
    , totalOpenSeconds : Int
    }


type alias ZonesResponse =
    { zones : List ZoneStatus
    }


type alias RunResult =
    { zoneId : String
    , openedForMinutes : Int
    }


type alias StopResult =
    { zoneId : String
    }


type alias Reading =
    { zoneId : String
    , kind : String
    , value : Float
    , takenAtMinutes : Int
    }



-- ── HTTP commands ─────────────────────────────────────────────────────────


fetchProperty : (Result Http.Error Property -> msg) -> Cmd msg
fetchProperty toMsg =
    Http.get
        { url = "/api/sim/property"
        , expect = Http.expectJson toMsg propertyDecoder
        }


fetchState : (Result Http.Error SimState -> msg) -> Cmd msg
fetchState toMsg =
    Http.get
        { url = "/api/sim/state"
        , expect = Http.expectJson toMsg simStateDecoder
        }


fetchZones : (Result Http.Error ZonesResponse -> msg) -> Cmd msg
fetchZones toMsg =
    Http.get
        { url = "/api/zones"
        , expect = Http.expectJson toMsg zonesResponseDecoder
        }


fetchWeather : (Result Http.Error Weather -> msg) -> Cmd msg
fetchWeather toMsg =
    Http.get
        { url = "/api/weather"
        , expect = Http.expectJson toMsg weatherDecoder
        }


fetchSensors : (Result Http.Error (List Reading) -> msg) -> Cmd msg
fetchSensors toMsg =
    Http.get
        { url = "/api/sensors"
        , expect = Http.expectJson toMsg sensorsResponseDecoder
        }


postStep : Int -> (Result Http.Error StepResult -> msg) -> Cmd msg
postStep minutes toMsg =
    Http.post
        { url = "/api/sim/step"
        , body =
            Http.jsonBody
                (Encode.object [ ( "minutes", Encode.int minutes ) ])
        , expect = Http.expectJson toMsg stepResultDecoder
        }


postReset : (Result Http.Error ResetResult -> msg) -> Cmd msg
postReset toMsg =
    Http.post
        { url = "/api/sim/reset"
        , body = Http.jsonBody (Encode.object [])
        , expect = Http.expectJson toMsg resetResultDecoder
        }


postRunZone : String -> Int -> (Result Http.Error RunResult -> msg) -> Cmd msg
postRunZone zoneId durationMinutes toMsg =
    Http.post
        { url = "/api/zones/" ++ zoneId ++ "/run"
        , body =
            Http.jsonBody
                (Encode.object
                    [ ( "duration_minutes", Encode.int durationMinutes ) ]
                )
        , expect = Http.expectJson toMsg runResultDecoder
        }


postStopZone : String -> (Result Http.Error StopResult -> msg) -> Cmd msg
postStopZone zoneId toMsg =
    Http.post
        { url = "/api/zones/" ++ zoneId ++ "/stop"
        , body = Http.jsonBody (Encode.object [])
        , expect = Http.expectJson toMsg stopResultDecoder
        }



-- ── Decoders ──────────────────────────────────────────────────────────────
--
-- Plain `mapN` decoders used here so the module sticks to the
-- elm/json package the project already pulls in; adding
-- `NoRedInk/elm-json-decode-pipeline` would be one more entry to
-- track in elm-srcs.nix for marginal readability.


propertyDecoder : Decoder Property
propertyDecoder =
    Decode.map7 Property
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "climate_zone" Decode.string)
        (Decode.field "lot_area_sq_ft" Decode.float)
        (Decode.field "yards" (Decode.list yardDecoder))
        (Decode.field "spigots" (Decode.list spigotDecoder))
        (Decode.field "zones" (Decode.list zoneDecoder))


yardDecoder : Decoder Yard
yardDecoder =
    Decode.map3 Yard
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "area_sq_ft" Decode.float)


spigotDecoder : Decoder Spigot
spigotDecoder =
    Decode.map3 Spigot
        (Decode.field "id" Decode.string)
        (Decode.field "mains_pressure_psi" Decode.float)
        (Decode.maybe (Decode.field "notes" Decode.string))


zoneDecoder : Decoder Zone
zoneDecoder =
    Decode.map5 Zone
        (Decode.field "id" Decode.string)
        (Decode.field "yard_id" Decode.string)
        (Decode.field "manifold_id" Decode.string)
        (Decode.field "plant_kind" Decode.string)
        (Decode.field "area_sq_ft" Decode.float)


simStateDecoder : Decoder SimState
simStateDecoder =
    Decode.map4 SimState
        (Decode.field "simulated_minutes_elapsed" Decode.int)
        (Decode.field "simulated_datetime" Decode.string)
        (Decode.field "zones" (Decode.list zoneStateDecoder))
        (Decode.field "weather" weatherDecoder)


zoneStateDecoder : Decoder ZoneState
zoneStateDecoder =
    Decode.map3 ZoneState
        (Decode.field "zone_id" Decode.string)
        (Decode.field "soil_vwc" Decode.float)
        (Decode.field "valve_is_open" Decode.bool)


weatherDecoder : Decoder Weather
weatherDecoder =
    Decode.map5 Weather
        (Decode.field "temperature_c" Decode.float)
        (Decode.field "humidity_pct" Decode.float)
        (Decode.field "wind_m_per_s" Decode.float)
        (Decode.field "solar_w_per_m2" Decode.float)
        (Decode.field "precipitation_mm_per_hour" Decode.float)


stepResultDecoder : Decoder StepResult
stepResultDecoder =
    Decode.map2 StepResult
        (Decode.field "minutes_advanced" Decode.int)
        (Decode.field "simulated_minutes_elapsed" Decode.int)


resetResultDecoder : Decoder ResetResult
resetResultDecoder =
    Decode.map ResetResult
        (Decode.field "reset_at" Decode.string)


zonesResponseDecoder : Decoder ZonesResponse
zonesResponseDecoder =
    Decode.map ZonesResponse
        (Decode.field "zones" (Decode.list zoneStatusDecoder))


zoneStatusDecoder : Decoder ZoneStatus
zoneStatusDecoder =
    Decode.map4 ZoneStatus
        (Decode.field "zone_id" Decode.string)
        (Decode.field "is_open" Decode.bool)
        (Decode.maybe (Decode.field "open_until_minutes" Decode.int))
        (Decode.field "total_open_seconds" Decode.int)


runResultDecoder : Decoder RunResult
runResultDecoder =
    Decode.map2 RunResult
        (Decode.field "zone_id" Decode.string)
        (Decode.field "opened_for_minutes" Decode.int)


stopResultDecoder : Decoder StopResult
stopResultDecoder =
    Decode.map StopResult
        (Decode.field "zone_id" Decode.string)


sensorsResponseDecoder : Decoder (List Reading)
sensorsResponseDecoder =
    Decode.field "readings" (Decode.list readingDecoder)


readingDecoder : Decoder Reading
readingDecoder =
    Decode.map4 Reading
        (Decode.field "zone_id" Decode.string)
        (Decode.field "kind" Decode.string)
        (Decode.field "value" Decode.float)
        (Decode.field "taken_at_minutes" Decode.int)
