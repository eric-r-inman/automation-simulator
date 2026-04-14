module Page.Dashboard exposing (Model, Msg, init, update, view)

{-| The simulator dashboard. One page that fetches property +
state + zones + weather on load, renders the SVG yard map, a list
of per-zone cards, the weather panel, and the simulation controls.
Every successful action triggers a re-fetch so the view stays in
sync with the engine.
-}

import Api
import Html exposing (Html, button, div, h1, h2, p, section, text)
import Html.Attributes as Attr
import Html.Events exposing (onClick)
import Http
import Widget.SimControls as SimControls
import Widget.Weather as Weather
import Widget.ZoneCard as ZoneCard



-- ── Model ─────────────────────────────────────────────────────────────────


type alias Model =
    { property : Loadable Api.Property
    , state : Loadable Api.SimState
    , zones : Loadable Api.ZonesResponse
    , busy : Bool
    , focusedZoneId : Maybe String
    , lastError : Maybe String
    }


type Loadable a
    = Loading
    | Loaded a
    | Failed String


init : ( Model, Cmd Msg )
init =
    ( { property = Loading
      , state = Loading
      , zones = Loading
      , busy = False
      , focusedZoneId = Nothing
      , lastError = Nothing
      }
    , refreshAll
    )



-- ── Messages ──────────────────────────────────────────────────────────────


type Msg
    = GotProperty (Result Http.Error Api.Property)
    | GotState (Result Http.Error Api.SimState)
    | GotZones (Result Http.Error Api.ZonesResponse)
    | RefreshClicked
    | StepClicked Int
    | StepResulted (Result Http.Error Api.StepResult)
    | ResetClicked
    | ResetResulted (Result Http.Error Api.ResetResult)
    | RunZoneClicked String Int
    | RunZoneResulted (Result Http.Error Api.RunResult)
    | StopZoneClicked String
    | StopZoneResulted (Result Http.Error Api.StopResult)
    | FocusZone String



-- ── Update ────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotProperty result ->
            ( { model | property = toLoadable result }, Cmd.none )

        GotState result ->
            ( { model | state = toLoadable result }, Cmd.none )

        GotZones result ->
            ( { model | zones = toLoadable result }, Cmd.none )

        RefreshClicked ->
            ( { model
                | property = Loading
                , state = Loading
                , zones = Loading
                , lastError = Nothing
              }
            , refreshAll
            )

        StepClicked minutes ->
            ( { model | busy = True, lastError = Nothing }
            , Api.postStep minutes StepResulted
            )

        StepResulted (Ok _) ->
            ( { model | busy = False }
            , refreshState
            )

        StepResulted (Err err) ->
            ( { model | busy = False, lastError = Just (httpErrorToString err) }
            , Cmd.none
            )

        ResetClicked ->
            ( { model | busy = True, lastError = Nothing }
            , Api.postReset ResetResulted
            )

        ResetResulted (Ok _) ->
            ( { model | busy = False }
            , refreshAll
            )

        ResetResulted (Err err) ->
            ( { model | busy = False, lastError = Just (httpErrorToString err) }
            , Cmd.none
            )

        RunZoneClicked zoneId minutes ->
            ( { model | busy = True, lastError = Nothing }
            , Api.postRunZone zoneId minutes RunZoneResulted
            )

        RunZoneResulted (Ok _) ->
            ( { model | busy = False }
            , refreshState
            )

        RunZoneResulted (Err err) ->
            ( { model | busy = False, lastError = Just (httpErrorToString err) }
            , Cmd.none
            )

        StopZoneClicked zoneId ->
            ( { model | busy = True, lastError = Nothing }
            , Api.postStopZone zoneId StopZoneResulted
            )

        StopZoneResulted (Ok _) ->
            ( { model | busy = False }
            , refreshState
            )

        StopZoneResulted (Err err) ->
            ( { model | busy = False, lastError = Just (httpErrorToString err) }
            , Cmd.none
            )

        FocusZone zoneId ->
            ( { model | focusedZoneId = Just zoneId }, Cmd.none )


refreshAll : Cmd Msg
refreshAll =
    Cmd.batch
        [ Api.fetchProperty GotProperty
        , Api.fetchState GotState
        , Api.fetchZones GotZones
        ]


refreshState : Cmd Msg
refreshState =
    Cmd.batch
        [ Api.fetchState GotState
        , Api.fetchZones GotZones
        ]


toLoadable : Result Http.Error a -> Loadable a
toLoadable result =
    case result of
        Ok value ->
            Loaded value

        Err err ->
            Failed (httpErrorToString err)


httpErrorToString : Http.Error -> String
httpErrorToString err =
    case err of
        Http.BadUrl u ->
            "bad URL: " ++ u

        Http.Timeout ->
            "request timed out"

        Http.NetworkError ->
            "network error — is the server running?"

        Http.BadStatus status ->
            "HTTP " ++ String.fromInt status

        Http.BadBody body ->
            "response body did not match the expected shape: " ++ body



-- ── View ──────────────────────────────────────────────────────────────────


view : Model -> Html Msg
view model =
    case ( model.property, model.state, model.zones ) of
        ( Loaded property, Loaded state, Loaded zonesResponse ) ->
            viewLoaded model property state zonesResponse

        ( Failed err, _, _ ) ->
            viewError "property" err

        ( _, Failed err, _ ) ->
            viewError "state" err

        ( _, _, Failed err ) ->
            viewError "zones" err

        _ ->
            div [ Attr.class "dashboard-loading" ]
                [ p [] [ text "Loading dashboard…" ] ]


viewLoaded :
    Model
    -> Api.Property
    -> Api.SimState
    -> Api.ZonesResponse
    -> Html Msg
viewLoaded model property state zonesResponse =
    let
        statusByZone : List ( String, Api.ZoneStatus )
        statusByZone =
            zonesResponse.zones
                |> List.map (\z -> ( z.zoneId, z ))

        lookupStatus : String -> Maybe Api.ZoneStatus
        lookupStatus zid =
            statusByZone
                |> List.filter (\( id, _ ) -> id == zid)
                |> List.head
                |> Maybe.map Tuple.second

        stateByZone : List ( String, Api.ZoneState )
        stateByZone =
            state.zones |> List.map (\z -> ( z.zoneId, z ))

        lookupState : String -> Maybe Api.ZoneState
        lookupState zid =
            stateByZone
                |> List.filter (\( id, _ ) -> id == zid)
                |> List.head
                |> Maybe.map Tuple.second

        zoneCards : List (Html Msg)
        zoneCards =
            property.zones
                |> List.map
                    (\zone ->
                        ZoneCard.view
                            { zone = zone
                            , state = lookupState zone.id
                            , status = lookupStatus zone.id
                            , onRun = RunZoneClicked zone.id
                            , onStop = StopZoneClicked zone.id
                            , onFocus = FocusZone zone.id
                            , isFocused = model.focusedZoneId == Just zone.id
                            }
                    )

        errorBanner : Html Msg
        errorBanner =
            case model.lastError of
                Just err ->
                    div [ Attr.class "error-banner" ]
                        [ text ("⚠ " ++ err) ]

                Nothing ->
                    text ""
    in
    div [ Attr.class "dashboard" ]
        [ section [ Attr.class "dashboard-header" ]
            [ h1 [] [ text property.name ]
            , p [ Attr.class "dashboard-subtitle" ]
                [ text
                    (property.climateZone
                        ++ " · "
                        ++ String.fromInt (List.length property.zones)
                        ++ " zones · "
                        ++ String.fromFloat property.lotAreaSqFt
                        ++ " sq ft"
                    )
                ]
            , button
                [ Attr.class "btn btn-refresh"
                , onClick RefreshClicked
                ]
                [ text "Refresh" ]
            ]
        , errorBanner
        , section [ Attr.class "dashboard-controls" ]
            [ SimControls.view
                { simulatedMinutes = state.simulatedMinutesElapsed
                , simulatedDatetime = state.simulatedDatetime
                , onStep = StepClicked
                , onReset = ResetClicked
                , isBusy = model.busy
                }
            , Weather.view state.weather
            ]
        , section [ Attr.class "dashboard-zones" ]
            [ h2 [] [ text "Zones" ]
            , div [ Attr.class "zone-grid" ] zoneCards
            ]
        ]


viewError : String -> String -> Html Msg
viewError what err =
    div [ Attr.class "dashboard-error" ]
        [ h1 [] [ text "Dashboard couldn’t load" ]
        , p []
            [ text ("Failed to fetch " ++ what ++ ": " ++ err) ]
        , button
            [ Attr.class "btn"
            , onClick RefreshClicked
            ]
            [ text "Try again" ]
        ]
