module Page.Properties exposing (Model, Msg, init, update, view)

{-| Properties list page.

Shows every property the server knows about in its in-memory
registry. The active one is highlighted; others get `Open`
(activate) and `Delete` buttons. A top link sends the user to
the Designer to create a new one.

-}

import Api
import Html exposing (Html, a, button, div, h1, li, p, section, span, text, ul)
import Html.Attributes as Attr
import Html.Events exposing (onClick)
import Http



-- ── Model ─────────────────────────────────────────────────────────────────


type alias Model =
    { data : Loadable Api.PropertiesListResponse
    , busy : Maybe ( String, Action )
    , notice : Maybe (Result String String)
    , confirmDelete : Maybe String
    }


type Action
    = Activating
    | Deleting


type Loadable a
    = Loading
    | Loaded a
    | Failed String


init : ( Model, Cmd Msg )
init =
    ( { data = Loading
      , busy = Nothing
      , notice = Nothing
      , confirmDelete = Nothing
      }
    , Api.fetchProperties GotList
    )



-- ── Messages ──────────────────────────────────────────────────────────────


type Msg
    = GotList (Result Http.Error Api.PropertiesListResponse)
    | ActivateClicked String
    | GotActivate (Result Http.Error Api.ActivatedProperty)
    | RequestDelete String
    | CancelDelete
    | ConfirmDelete String
    | GotDelete (Result Http.Error Api.DeletedResult)



-- ── Update ────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotList (Ok r) ->
            ( { model | data = Loaded r }, Cmd.none )

        GotList (Err e) ->
            ( { model | data = Failed (httpErrToString e) }, Cmd.none )

        ActivateClicked id ->
            ( { model | busy = Just ( id, Activating ), notice = Nothing }
            , Api.postActivateProperty id GotActivate
            )

        GotActivate (Ok ap) ->
            ( { model
                | busy = Nothing
                , notice =
                    Just
                        (Ok
                            ("Simulator now running "
                                ++ ap.propertyName
                                ++ " ("
                                ++ String.fromInt ap.zones
                                ++ " zones)."
                            )
                        )
              }
            , Api.fetchProperties GotList
            )

        GotActivate (Err e) ->
            ( { model | busy = Nothing, notice = Just (Err (httpErrToString e)) }
            , Cmd.none
            )

        RequestDelete id ->
            ( { model | confirmDelete = Just id, notice = Nothing }, Cmd.none )

        CancelDelete ->
            ( { model | confirmDelete = Nothing }, Cmd.none )

        ConfirmDelete id ->
            ( { model
                | busy = Just ( id, Deleting )
                , confirmDelete = Nothing
                , notice = Nothing
              }
            , Api.deleteProperty id GotDelete
            )

        GotDelete (Ok r) ->
            ( { model
                | busy = Nothing
                , notice = Just (Ok ("Removed " ++ r.zoneId ++ "."))
              }
            , Api.fetchProperties GotList
            )

        GotDelete (Err e) ->
            ( { model | busy = Nothing, notice = Just (Err (httpErrToString e)) }
            , Cmd.none
            )


httpErrToString : Http.Error -> String
httpErrToString err =
    case err of
        Http.BadUrl u ->
            "Bad URL: " ++ u

        Http.Timeout ->
            "Request timed out."

        Http.NetworkError ->
            "Network error."

        Http.BadStatus status ->
            "Server returned status " ++ String.fromInt status ++ "."

        Http.BadBody m ->
            "Unexpected response body: " ++ m



-- ── View ──────────────────────────────────────────────────────────────────


view : Model -> Html Msg
view model =
    div [ Attr.class "properties-page" ]
        [ h1 [] [ text "Properties" ]
        , p [ Attr.class "page-intro" ]
            [ text "Every property this server has seen this session.  "
            , text "Open one to point the simulator at it."
            ]
        , div [ Attr.class "properties-actions" ]
            [ a
                [ Attr.href "/designer"
                , Attr.class "btn btn-primary"
                ]
                [ text "+ New property (Designer)" ]
            ]
        , viewNotice model.notice
        , viewList model
        ]


viewNotice : Maybe (Result String String) -> Html Msg
viewNotice notice =
    case notice of
        Nothing ->
            text ""

        Just (Ok m) ->
            div [ Attr.class "plan-apply-ok" ] [ text m ]

        Just (Err m) ->
            div [ Attr.class "plan-apply-err" ] [ text m ]


viewList : Model -> Html Msg
viewList model =
    case model.data of
        Loading ->
            p [] [ text "Loading…" ]

        Failed m ->
            div [ Attr.class "plans-failed" ] [ p [] [ text m ] ]

        Loaded r ->
            if List.isEmpty r.properties then
                p [] [ text "No properties registered yet." ]

            else
                ul [ Attr.class "property-list" ]
                    (List.map (viewProperty model) r.properties)


viewProperty : Model -> Api.PropertyListEntry -> Html Msg
viewProperty model p =
    let
        activeBadge : Html Msg
        activeBadge =
            if p.active then
                span [ Attr.class "badge badge-active" ] [ text "ACTIVE" ]

            else
                text ""

        busyMsg : Maybe Action
        busyMsg =
            case model.busy of
                Just ( bid, action ) ->
                    if bid == p.id then
                        Just action

                    else
                        Nothing

                Nothing ->
                    Nothing

        openBtn : Html Msg
        openBtn =
            if p.active then
                a
                    [ Attr.href "/"
                    , Attr.class "btn btn-primary"
                    ]
                    [ text "Dashboard →" ]

            else
                button
                    [ Attr.class "btn btn-primary"
                    , Attr.disabled (model.busy /= Nothing)
                    , onClick (ActivateClicked p.id)
                    ]
                    [ text
                        (case busyMsg of
                            Just Activating ->
                                "Opening…"

                            _ ->
                                "Open"
                        )
                    ]

        deleteBtns : Html Msg
        deleteBtns =
            case model.confirmDelete of
                Just cid ->
                    if cid == p.id then
                        span [ Attr.class "confirm-delete" ]
                            [ text "Delete? "
                            , button
                                [ Attr.class "btn btn-remove"
                                , onClick (ConfirmDelete p.id)
                                ]
                                [ text "Yes" ]
                            , text " "
                            , button
                                [ Attr.class "btn"
                                , onClick CancelDelete
                                ]
                                [ text "No" ]
                            ]

                    else
                        text ""

                Nothing ->
                    if p.active then
                        text ""

                    else
                        button
                            [ Attr.class "btn btn-remove"
                            , Attr.disabled (model.busy /= Nothing)
                            , onClick (RequestDelete p.id)
                            ]
                            [ text
                                (case busyMsg of
                                    Just Deleting ->
                                        "Deleting…"

                                    _ ->
                                        "Delete"
                                )
                            ]
    in
    li [ Attr.class "property-row" ]
        [ div [ Attr.class "property-head" ]
            [ span [ Attr.class "property-name" ] [ text p.name ]
            , text " "
            , span [ Attr.class "property-id" ] [ text ("(" ++ p.id ++ ")") ]
            , text " "
            , activeBadge
            ]
        , div [ Attr.class "property-meta" ]
            [ text
                (String.fromInt p.zones
                    ++ " zones · "
                    ++ String.fromInt p.yards
                    ++ " yards · "
                    ++ p.climateZone
                )
            ]
        , div [ Attr.class "property-actions" ]
            [ openBtn
            , text " "
            , deleteBtns
            ]
        ]
