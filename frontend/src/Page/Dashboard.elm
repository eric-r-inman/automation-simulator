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
import Widget.ZoneEditor as ZoneEditor



-- ── Model ─────────────────────────────────────────────────────────────────


type alias Model =
    { property : Loadable Api.Property
    , state : Loadable Api.SimState
    , zones : Loadable Api.ZonesResponse
    , catalog : Loadable Api.Catalog
    , busy : Bool
    , focusedZoneId : Maybe String
    , lastError : Maybe String
    , editor : Maybe ZoneEditor.Form
    , confirmDeleteZoneId : Maybe String
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
      , catalog = Loading
      , busy = False
      , focusedZoneId = Nothing
      , lastError = Nothing
      , editor = Nothing
      , confirmDeleteZoneId = Nothing
      }
    , refreshAll
    )



-- ── Messages ──────────────────────────────────────────────────────────────


type Msg
    = GotProperty (Result Http.Error Api.Property)
    | GotState (Result Http.Error Api.SimState)
    | GotZones (Result Http.Error Api.ZonesResponse)
    | GotCatalog (Result Http.Error Api.Catalog)
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
    | OpenAddZone
    | OpenEditZone Api.Zone
    | EditorMsg ZoneEditor.Msg
    | EditorSubmit
    | EditorCancel
    | ZoneCreated (Result Http.Error Api.ZoneDefinition)
    | ZoneUpdated (Result Http.Error Api.ZoneDefinition)
    | RequestDeleteZone String
    | CancelDelete
    | ConfirmDeleteZone String
    | ZoneDeleted (Result Http.Error Api.DeletedResult)



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

        GotCatalog result ->
            ( { model | catalog = toLoadable result }, Cmd.none )

        OpenAddZone ->
            case ( model.property, model.catalog ) of
                ( Loaded p, Loaded c ) ->
                    ( { model
                        | editor = Just (ZoneEditor.init ZoneEditor.Create Nothing p c)
                        , lastError = Nothing
                      }
                    , Cmd.none
                    )

                _ ->
                    ( { model | lastError = Just "Property + catalog must finish loading first." }
                    , Cmd.none
                    )

        OpenEditZone zone ->
            case ( model.property, model.catalog ) of
                ( Loaded p, Loaded c ) ->
                    let
                        existing : Api.ZoneDefinition
                        existing =
                            { id = zone.id
                            , yardId = zone.yardId
                            , manifoldId = zone.manifoldId
                            , plantKind = zone.plantKind
                            , emitterSpecId = ""
                            , soilTypeId = ""
                            , areaSqFt = zone.areaSqFt
                            , notes = Nothing
                            }
                    in
                    -- Fetch the full definition so the editor opens with
                    -- the *current* emitter/soil/notes, not the trimmed
                    -- summary the property endpoint returns.
                    ( { model
                        | editor = Just (ZoneEditor.init (ZoneEditor.Edit zone.id) (Just existing) p c)
                        , lastError = Nothing
                      }
                    , Cmd.batch [ fetchZoneDefinition zone.id p c ]
                    )

                _ ->
                    ( { model | lastError = Just "Property + catalog must finish loading first." }
                    , Cmd.none
                    )

        EditorMsg emsg ->
            ( { model | editor = Maybe.map (ZoneEditor.update emsg) model.editor }
            , Cmd.none
            )

        EditorSubmit ->
            case model.editor of
                Just f ->
                    case ZoneEditor.submit f of
                        Err errMsg ->
                            ( { model
                                | editor = Just { f | validationError = Just errMsg }
                              }
                            , Cmd.none
                            )

                        Ok (Err createReq) ->
                            ( { model | busy = True }
                            , Api.postCreateZone createReq ZoneCreated
                            )

                        Ok (Ok ( zoneId, updateReq )) ->
                            ( { model | busy = True }
                            , Api.postUpdateZone zoneId updateReq ZoneUpdated
                            )

                Nothing ->
                    ( model, Cmd.none )

        EditorCancel ->
            ( { model | editor = Nothing }, Cmd.none )

        ZoneCreated (Ok _) ->
            ( { model | busy = False, editor = Nothing }
            , refreshAll
            )

        ZoneCreated (Err err) ->
            ( { model
                | busy = False
                , editor =
                    Maybe.map
                        (\f -> { f | validationError = Just (httpErrorToString err) })
                        model.editor
              }
            , Cmd.none
            )

        ZoneUpdated (Ok _) ->
            ( { model | busy = False, editor = Nothing }
            , refreshAll
            )

        ZoneUpdated (Err err) ->
            ( { model
                | busy = False
                , editor =
                    Maybe.map
                        (\f -> { f | validationError = Just (httpErrorToString err) })
                        model.editor
              }
            , Cmd.none
            )

        RequestDeleteZone zoneId ->
            ( { model | confirmDeleteZoneId = Just zoneId, lastError = Nothing }
            , Cmd.none
            )

        CancelDelete ->
            ( { model | confirmDeleteZoneId = Nothing }, Cmd.none )

        ConfirmDeleteZone zoneId ->
            ( { model | busy = True, confirmDeleteZoneId = Nothing }
            , Api.deleteZone zoneId ZoneDeleted
            )

        ZoneDeleted (Ok _) ->
            ( { model | busy = False }
            , refreshAll
            )

        ZoneDeleted (Err err) ->
            ( { model | busy = False, lastError = Just (httpErrorToString err) }
            , Cmd.none
            )


fetchZoneDefinition : String -> Api.Property -> Api.Catalog -> Cmd Msg
fetchZoneDefinition _ _ _ =
    -- Hook for Phase 12 follow-up: fetch the full zone definition
    -- via /api/zones/definitions/{id} and refresh the editor with
    -- accurate emitter/soil ids.  v0.1 leaves the editor's
    -- defaults in place and relies on the user picking from the
    -- dropdowns.
    Cmd.none


refreshAll : Cmd Msg
refreshAll =
    Cmd.batch
        [ Api.fetchProperty GotProperty
        , Api.fetchState GotState
        , Api.fetchZones GotZones
        , Api.fetchCatalog GotCatalog
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
                            , onEdit = OpenEditZone zone
                            , onDelete = RequestDeleteZone zone.id
                            , isFocused = model.focusedZoneId == Just zone.id
                            }
                    )

        editorOverlay : Html Msg
        editorOverlay =
            case ( model.editor, model.catalog ) of
                ( Just f, Loaded c ) ->
                    ZoneEditor.view
                        { form = f
                        , property = property
                        , catalog = c
                        , onMsg = EditorMsg
                        , onSubmit = EditorSubmit
                        , onCancel = EditorCancel
                        , isBusy = model.busy
                        }

                _ ->
                    text ""

        deleteConfirm : Html Msg
        deleteConfirm =
            case model.confirmDeleteZoneId of
                Just zid ->
                    div [ Attr.class "modal-backdrop" ]
                        [ div [ Attr.class "modal modal-confirm" ]
                            [ h2 [] [ text "Delete zone?" ]
                            , p []
                                [ text "Removing "
                                , Html.strong [] [ text zid ]
                                , text " also clears its valve state and recorded history. This cannot be undone."
                                ]
                            , div [ Attr.class "form-actions" ]
                                [ button
                                    [ Attr.class "btn"
                                    , onClick CancelDelete
                                    ]
                                    [ text "Cancel" ]
                                , button
                                    [ Attr.class "btn btn-delete"
                                    , Attr.disabled model.busy
                                    , onClick (ConfirmDeleteZone zid)
                                    ]
                                    [ text "Delete" ]
                                ]
                            ]
                        ]

                Nothing ->
                    text ""

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
            [ div [ Attr.class "zones-header" ]
                [ h2 [] [ text "Zones" ]
                , button
                    [ Attr.class "btn btn-run"
                    , onClick OpenAddZone
                    ]
                    [ text "+ Add zone" ]
                ]
            , div [ Attr.class "zone-grid" ] zoneCards
            ]
        , editorOverlay
        , deleteConfirm
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
