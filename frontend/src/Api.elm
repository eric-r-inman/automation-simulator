module Api exposing
    ( BomLine
    , Catalog
    , CatalogEmitter
    , CatalogSoilType
    , CatalogSpecies
    , DeletedResult
    , Manifold
    , Plan
    , PlanBom
    , PlanRequest
    , PlanResponse
    , PlanYardInput
    , PlanZoneInput
    , Property
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
    , ZoneCreateRequest
    , ZoneDefinition
    , ZoneState
    , ZoneStatus
    , ZoneUpdateRequest
    , ZonesResponse
    , deleteZone
    , emptyZoneUpdate
    , fetchCatalog
    , fetchProperty
    , fetchSensors
    , fetchState
    , fetchWeather
    , fetchZones
    , postCreateZone
    , postPlan
    , postReset
    , postRunZone
    , postStep
    , postStopZone
    , postUpdateZone
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
    , manifolds : List Manifold
    , zones : List Zone
    }


type alias Manifold =
    { id : String
    , modelId : String
    , spigotId : String
    , zoneCapacity : Int
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


type alias Catalog =
    { emitters : List CatalogEmitter
    , soilTypes : List CatalogSoilType
    , species : List CatalogSpecies
    }


type alias CatalogEmitter =
    { id : String
    , name : String
    , manufacturer : String
    , shape : String
    , flowGph : Float
    , pressureCompensating : Bool
    }


type alias CatalogSoilType =
    { id : String
    , name : String
    , fieldCapacityVwc : Float
    , wiltingPointVwc : Float
    }


type alias CatalogSpecies =
    { id : String
    , commonName : String
    , scientificName : String
    , plantKind : String
    , waterNeedBaseMlPerDay : Float
    }


type alias ZoneDefinition =
    { id : String
    , yardId : String
    , manifoldId : String
    , plantKind : String
    , emitterSpecId : String
    , soilTypeId : String
    , areaSqFt : Float
    , notes : Maybe String
    }


type alias ZoneCreateRequest =
    { id : String
    , yardId : String
    , manifoldId : String
    , plantKind : String
    , emitterSpecId : String
    , soilTypeId : String
    , areaSqFt : Float
    , notes : Maybe String
    }


type alias ZoneUpdateRequest =
    { yardId : Maybe String
    , manifoldId : Maybe String
    , plantKind : Maybe String
    , emitterSpecId : Maybe String
    , soilTypeId : Maybe String
    , areaSqFt : Maybe Float
    , notes : Maybe String
    }


emptyZoneUpdate : ZoneUpdateRequest
emptyZoneUpdate =
    { yardId = Nothing
    , manifoldId = Nothing
    , plantKind = Nothing
    , emitterSpecId = Nothing
    , soilTypeId = Nothing
    , areaSqFt = Nothing
    , notes = Nothing
    }


type alias DeletedResult =
    { zoneId : String
    }



-- ── Planner types ─────────────────────────────────────────────────────────


type alias PlanRequest =
    { propertyId : String
    , propertyName : String
    , climateZone : String
    , yards : List PlanYardInput
    , budgetUsd : Maybe Float
    , preferSmartController : Bool
    , requirePressureCompensating : Bool
    , soilTypeId : String
    , topN : Int
    }


type alias PlanYardInput =
    { id : String
    , name : String
    , areaSqFt : Float
    , mainsPressurePsi : Float
    , zones : List PlanZoneInput
    }


type alias PlanZoneInput =
    { nameSuffix : String
    , plantKind : String
    , areaSqFt : Float
    }


type alias PlanResponse =
    { plans : List Plan
    }


type alias Plan =
    { planId : String
    , controllerModelId : String
    , controllerMaxZones : Int
    , score : Float
    , rationale : List String
    , bom : PlanBom
    }


type alias PlanBom =
    { lines : List BomLine
    , totalUsd : Float
    }


type alias BomLine =
    { category : String
    , catalogId : String
    , displayName : String
    , manufacturer : String
    , quantity : Int
    , unitPriceUsd : Float
    , lineTotalUsd : Float
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


fetchCatalog : (Result Http.Error Catalog -> msg) -> Cmd msg
fetchCatalog toMsg =
    Http.get
        { url = "/api/catalog"
        , expect = Http.expectJson toMsg catalogDecoder
        }


postCreateZone : ZoneCreateRequest -> (Result Http.Error ZoneDefinition -> msg) -> Cmd msg
postCreateZone req toMsg =
    Http.post
        { url = "/api/zones/definitions"
        , body = Http.jsonBody (zoneCreateEncoder req)
        , expect = Http.expectJson toMsg zoneDefinitionDecoder
        }


postUpdateZone : String -> ZoneUpdateRequest -> (Result Http.Error ZoneDefinition -> msg) -> Cmd msg
postUpdateZone zoneId req toMsg =
    -- elm/http exposes Http.request rather than a dedicated Http.patch.
    Http.request
        { method = "PATCH"
        , headers = []
        , url = "/api/zones/definitions/" ++ zoneId
        , body = Http.jsonBody (zoneUpdateEncoder req)
        , expect = Http.expectJson toMsg zoneDefinitionDecoder
        , timeout = Nothing
        , tracker = Nothing
        }


postPlan : PlanRequest -> (Result Http.Error PlanResponse -> msg) -> Cmd msg
postPlan req toMsg =
    Http.post
        { url = "/api/plan"
        , body = Http.jsonBody (planRequestEncoder req)
        , expect = Http.expectJson toMsg planResponseDecoder
        }


planRequestEncoder : PlanRequest -> Encode.Value
planRequestEncoder req =
    let
        budget =
            case req.budgetUsd of
                Just b ->
                    [ ( "budget_usd", Encode.float b ) ]

                Nothing ->
                    []
    in
    Encode.object
        ([ ( "property_id", Encode.string req.propertyId )
         , ( "property_name", Encode.string req.propertyName )
         , ( "climate_zone", Encode.string req.climateZone )
         , ( "yards", Encode.list planYardEncoder req.yards )
         , ( "prefer_smart_controller", Encode.bool req.preferSmartController )
         , ( "require_pressure_compensating", Encode.bool req.requirePressureCompensating )
         , ( "soil_type_id", Encode.string req.soilTypeId )
         , ( "top_n", Encode.int req.topN )
         ]
            ++ budget
        )


planYardEncoder : PlanYardInput -> Encode.Value
planYardEncoder y =
    Encode.object
        [ ( "id", Encode.string y.id )
        , ( "name", Encode.string y.name )
        , ( "area_sq_ft", Encode.float y.areaSqFt )
        , ( "mains_pressure_psi", Encode.float y.mainsPressurePsi )
        , ( "zones", Encode.list planZoneEncoder y.zones )
        ]


planZoneEncoder : PlanZoneInput -> Encode.Value
planZoneEncoder z =
    Encode.object
        [ ( "name_suffix", Encode.string z.nameSuffix )
        , ( "plant_kind", Encode.string z.plantKind )
        , ( "area_sq_ft", Encode.float z.areaSqFt )
        ]


deleteZone : String -> (Result Http.Error DeletedResult -> msg) -> Cmd msg
deleteZone zoneId toMsg =
    Http.request
        { method = "DELETE"
        , headers = []
        , url = "/api/zones/definitions/" ++ zoneId
        , body = Http.emptyBody
        , expect = Http.expectJson toMsg deletedResultDecoder
        , timeout = Nothing
        , tracker = Nothing
        }


zoneCreateEncoder : ZoneCreateRequest -> Encode.Value
zoneCreateEncoder req =
    let
        notes =
            case req.notes of
                Just s ->
                    [ ( "notes", Encode.string s ) ]

                Nothing ->
                    []
    in
    Encode.object
        ([ ( "id", Encode.string req.id )
         , ( "yard_id", Encode.string req.yardId )
         , ( "manifold_id", Encode.string req.manifoldId )
         , ( "plant_kind", Encode.string req.plantKind )
         , ( "emitter_spec_id", Encode.string req.emitterSpecId )
         , ( "soil_type_id", Encode.string req.soilTypeId )
         , ( "area_sq_ft", Encode.float req.areaSqFt )
         ]
            ++ notes
        )


zoneUpdateEncoder : ZoneUpdateRequest -> Encode.Value
zoneUpdateEncoder req =
    let
        maybeField : String -> Maybe a -> (a -> Encode.Value) -> List ( String, Encode.Value )
        maybeField key m enc =
            case m of
                Just v ->
                    [ ( key, enc v ) ]

                Nothing ->
                    []
    in
    Encode.object
        (maybeField "yard_id" req.yardId Encode.string
            ++ maybeField "manifold_id" req.manifoldId Encode.string
            ++ maybeField "plant_kind" req.plantKind Encode.string
            ++ maybeField "emitter_spec_id" req.emitterSpecId Encode.string
            ++ maybeField "soil_type_id" req.soilTypeId Encode.string
            ++ maybeField "area_sq_ft" req.areaSqFt Encode.float
            ++ maybeField "notes" req.notes Encode.string
        )



-- ── Decoders ──────────────────────────────────────────────────────────────
--
-- Plain `mapN` decoders used here so the module sticks to the
-- elm/json package the project already pulls in; adding
-- `NoRedInk/elm-json-decode-pipeline` would be one more entry to
-- track in elm-srcs.nix for marginal readability.


propertyDecoder : Decoder Property
propertyDecoder =
    Decode.map8 Property
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "climate_zone" Decode.string)
        (Decode.field "lot_area_sq_ft" Decode.float)
        (Decode.field "yards" (Decode.list yardDecoder))
        (Decode.field "spigots" (Decode.list spigotDecoder))
        (Decode.field "manifolds" (Decode.list manifoldDecoder))
        (Decode.field "zones" (Decode.list zoneDecoder))


manifoldDecoder : Decoder Manifold
manifoldDecoder =
    Decode.map4 Manifold
        (Decode.field "id" Decode.string)
        (Decode.field "model_id" Decode.string)
        (Decode.field "spigot_id" Decode.string)
        (Decode.field "zone_capacity" Decode.int)


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


catalogDecoder : Decoder Catalog
catalogDecoder =
    Decode.map3 Catalog
        (Decode.field "emitters" (Decode.list emitterDecoder))
        (Decode.field "soil_types" (Decode.list soilTypeDecoder))
        (Decode.field "species" (Decode.list speciesDecoder))


emitterDecoder : Decoder CatalogEmitter
emitterDecoder =
    Decode.map6 CatalogEmitter
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "manufacturer" Decode.string)
        (Decode.field "shape" Decode.string)
        (Decode.field "flow_gph" Decode.float)
        (Decode.field "pressure_compensating" Decode.bool)


soilTypeDecoder : Decoder CatalogSoilType
soilTypeDecoder =
    Decode.map4 CatalogSoilType
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "field_capacity_vwc" Decode.float)
        (Decode.field "wilting_point_vwc" Decode.float)


speciesDecoder : Decoder CatalogSpecies
speciesDecoder =
    Decode.map5 CatalogSpecies
        (Decode.field "id" Decode.string)
        (Decode.field "common_name" Decode.string)
        (Decode.field "scientific_name" Decode.string)
        (Decode.field "plant_kind" Decode.string)
        (Decode.field "water_need_base_ml_per_day" Decode.float)


zoneDefinitionDecoder : Decoder ZoneDefinition
zoneDefinitionDecoder =
    Decode.map8 ZoneDefinition
        (Decode.field "id" Decode.string)
        (Decode.field "yard_id" Decode.string)
        (Decode.field "manifold_id" Decode.string)
        (Decode.field "plant_kind" Decode.string)
        (Decode.field "emitter_spec_id" Decode.string)
        (Decode.field "soil_type_id" Decode.string)
        (Decode.field "area_sq_ft" Decode.float)
        (Decode.maybe (Decode.field "notes" Decode.string))


deletedResultDecoder : Decoder DeletedResult
deletedResultDecoder =
    Decode.map DeletedResult
        (Decode.field "zone_id" Decode.string)


planResponseDecoder : Decoder PlanResponse
planResponseDecoder =
    Decode.map PlanResponse
        (Decode.field "plans" (Decode.list planDecoder))


planDecoder : Decoder Plan
planDecoder =
    Decode.map6 Plan
        (Decode.field "plan_id" Decode.string)
        (Decode.field "controller_model_id" Decode.string)
        (Decode.field "controller_max_zones" Decode.int)
        (Decode.field "score" Decode.float)
        (Decode.field "rationale" (Decode.list Decode.string))
        (Decode.field "bom" planBomDecoder)


planBomDecoder : Decoder PlanBom
planBomDecoder =
    Decode.map2 PlanBom
        (Decode.field "lines" (Decode.list bomLineDecoder))
        (Decode.field "total_usd" Decode.float)


bomLineDecoder : Decoder BomLine
bomLineDecoder =
    Decode.map7 BomLine
        (Decode.field "category" Decode.string)
        (Decode.field "catalog_id" Decode.string)
        (Decode.field "display_name" Decode.string)
        (Decode.field "manufacturer" Decode.string)
        (Decode.field "quantity" Decode.int)
        (Decode.field "unit_price_usd" Decode.float)
        (Decode.field "line_total_usd" Decode.float)
