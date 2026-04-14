module Main exposing (main)

{-| Top-level shell + URL routing.

Two routes for v0.1: the dashboard at `/` (everything the
homeowner needs) and the legacy `/me` page from the template
(kept so the OIDC flow still has a landing page when authentication
is enabled). `Main.elm` stays a thin orchestrator: it owns the
URL key, decodes the URL into a route, holds whichever page model
applies, and forwards messages to the page's `update` /`view`.

-}

import Browser
import Browser.Navigation as Nav
import Html exposing (Html, a, div, h1, main_, nav, p, text)
import Html.Attributes as Attr
import Http
import Json.Decode as Decode
import Page.Dashboard as Dashboard
import Page.Designer as Designer
import Page.Properties as Properties
import Url exposing (Url)
import Url.Parser exposing (Parser, oneOf, top)



-- ── Routing ───────────────────────────────────────────────────────────────


type Route
    = Dashboard
    | DesignerRoute
    | PropertiesRoute
    | Me
    | NotFound


routeParser : Parser (Route -> a) a
routeParser =
    oneOf
        [ Url.Parser.map Dashboard top
        , Url.Parser.map DesignerRoute (Url.Parser.s "designer")
        , Url.Parser.map PropertiesRoute (Url.Parser.s "properties")
        , Url.Parser.map Me (Url.Parser.s "me")
        ]


routeFromUrl : Url -> Route
routeFromUrl url =
    Url.Parser.parse routeParser url
        |> Maybe.withDefault NotFound



-- ── Me page (kept for OIDC) ───────────────────────────────────────────────


type alias MeInfo =
    { name : String
    , authEnabled : Bool
    }


type MeStatus
    = MeLoading
    | MeLoaded MeInfo
    | MeFailed



-- ── Page model union ──────────────────────────────────────────────────────


type Page
    = DashboardPage Dashboard.Model
    | DesignerPage Designer.Model
    | PropertiesPage Properties.Model
    | MePage MeStatus
    | NotFoundPage



-- ── Top-level model ───────────────────────────────────────────────────────


type alias Model =
    { key : Nav.Key
    , url : Url
    , page : Page
    }


type Msg
    = UrlRequested Browser.UrlRequest
    | UrlChanged Url
    | DashboardMsg Dashboard.Msg
    | DesignerMsg Designer.Msg
    | PropertiesMsg Properties.Msg
    | GotMe (Result Http.Error MeInfo)


main : Program () Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = \_ -> Sub.none
        , onUrlRequest = UrlRequested
        , onUrlChange = UrlChanged
        }


init : () -> Url -> Nav.Key -> ( Model, Cmd Msg )
init _ url key =
    let
        ( page, cmd ) =
            initPage (routeFromUrl url)
    in
    ( { key = key, url = url, page = page }
    , cmd
    )


initPage : Route -> ( Page, Cmd Msg )
initPage route =
    case route of
        Dashboard ->
            let
                ( m, c ) =
                    Dashboard.init
            in
            ( DashboardPage m, Cmd.map DashboardMsg c )

        DesignerRoute ->
            let
                ( m, c ) =
                    Designer.init
            in
            ( DesignerPage m, Cmd.map DesignerMsg c )

        PropertiesRoute ->
            let
                ( m, c ) =
                    Properties.init
            in
            ( PropertiesPage m, Cmd.map PropertiesMsg c )

        Me ->
            ( MePage MeLoading, fetchMe )

        NotFound ->
            ( NotFoundPage, Cmd.none )


fetchMe : Cmd Msg
fetchMe =
    Http.get
        { url = "/me"
        , expect = Http.expectJson GotMe meDecoder
        }


meDecoder : Decode.Decoder MeInfo
meDecoder =
    Decode.map2 MeInfo
        (Decode.field "name" Decode.string)
        (Decode.field "auth_enabled" Decode.bool)



-- ── Update ────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        UrlRequested (Browser.Internal url) ->
            ( model, Nav.pushUrl model.key (Url.toString url) )

        UrlRequested (Browser.External url) ->
            ( model, Nav.load url )

        UrlChanged url ->
            let
                ( page, cmd ) =
                    initPage (routeFromUrl url)
            in
            ( { model | url = url, page = page }, cmd )

        DashboardMsg dmsg ->
            case model.page of
                DashboardPage dmodel ->
                    let
                        ( newModel, cmd ) =
                            Dashboard.update dmsg dmodel
                    in
                    ( { model | page = DashboardPage newModel }
                    , Cmd.map DashboardMsg cmd
                    )

                _ ->
                    ( model, Cmd.none )

        DesignerMsg dmsg ->
            case model.page of
                DesignerPage dmodel ->
                    let
                        ( newModel, cmd ) =
                            Designer.update dmsg dmodel
                    in
                    ( { model | page = DesignerPage newModel }
                    , Cmd.map DesignerMsg cmd
                    )

                _ ->
                    ( model, Cmd.none )

        PropertiesMsg pmsg ->
            case model.page of
                PropertiesPage pmodel ->
                    let
                        ( newModel, cmd ) =
                            Properties.update pmsg pmodel
                    in
                    ( { model | page = PropertiesPage newModel }
                    , Cmd.map PropertiesMsg cmd
                    )

                _ ->
                    ( model, Cmd.none )

        GotMe result ->
            case model.page of
                MePage _ ->
                    let
                        next : MeStatus
                        next =
                            case result of
                                Ok info ->
                                    MeLoaded info

                                Err _ ->
                                    MeFailed
                    in
                    ( { model | page = MePage next }, Cmd.none )

                _ ->
                    ( model, Cmd.none )



-- ── View ──────────────────────────────────────────────────────────────────


view : Model -> Browser.Document Msg
view model =
    { title = "automation-simulator"
    , body =
        [ main_ [ Attr.class "site" ]
            [ nav [ Attr.class "site-nav" ]
                [ a [ Attr.href "/" ] [ text "Dashboard" ]
                , text " · "
                , a [ Attr.href "/properties" ] [ text "Properties" ]
                , text " · "
                , a [ Attr.href "/designer" ] [ text "Designer" ]
                , text " · "
                , a [ Attr.href "/me" ] [ text "Me" ]
                , text " · "
                , a [ Attr.href "/scalar" ] [ text "API docs" ]
                ]
            , viewPage model
            ]
        ]
    }


viewPage : Model -> Html Msg
viewPage model =
    case model.page of
        DashboardPage dmodel ->
            Html.map DashboardMsg (Dashboard.view dmodel)

        DesignerPage dmodel ->
            Html.map DesignerMsg (Designer.view dmodel)

        PropertiesPage pmodel ->
            Html.map PropertiesMsg (Properties.view pmodel)

        MePage status ->
            viewMe status

        NotFoundPage ->
            div [ Attr.class "not-found" ]
                [ h1 [] [ text "Not found" ]
                , p [] [ text "The page you requested does not exist." ]
                ]


viewMe : MeStatus -> Html Msg
viewMe status =
    case status of
        MeLoading ->
            p [] [ text "Loading…" ]

        MeFailed ->
            p [] [ text "Failed to load user information." ]

        MeLoaded info ->
            div [ Attr.class "me-page" ]
                [ h1 [] [ text "Me" ]
                , p [] [ text ("Name: " ++ info.name) ]
                , p []
                    [ text
                        ("Authentication: "
                            ++ (if info.authEnabled then
                                    "enabled"

                                else
                                    "disabled"
                               )
                        )
                    ]
                ]
