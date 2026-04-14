module Widget.ZoneEditor exposing
    ( EditorMode(..)
    , Form
    , Msg
    , init
    , submit
    , update
    , view
    )

{-| Modal-style form for creating or editing a zone.

Renders as an overlay panel. `init` builds the initial form data
from either an existing `ZoneDefinition` (Edit mode) or empty
strings + sensible defaults (Create mode). `update` handles
field edits. `submit` extracts a typed `ZoneCreateRequest` (or
`ZoneUpdateRequest`) ready to hand to `Api.postCreateZone` /
`Api.postUpdateZone`; the dashboard owns the actual HTTP call.

The editor depends on the loaded `Property` (for yard + manifold
dropdowns) and the loaded `Catalog` (for emitter + soil-type
dropdowns). It does not fetch them itself — the dashboard
already has them and passes them in.

-}

import Api
import Html exposing (Html, button, div, form, h2, input, label, option, p, select, text, textarea)
import Html.Attributes as Attr
import Html.Events exposing (onClick, onInput, onSubmit)


type EditorMode
    = Create
    | Edit String


type alias Form =
    { mode : EditorMode
    , id : String
    , yardId : String
    , manifoldId : String
    , plantKind : String
    , emitterSpecId : String
    , soilTypeId : String
    , areaSqFt : String
    , notes : String
    , validationError : Maybe String
    }


init : EditorMode -> Maybe Api.ZoneDefinition -> Api.Property -> Api.Catalog -> Form
init mode existing property catalog =
    let
        firstYard : String
        firstYard =
            property.yards |> List.head |> Maybe.map .id |> Maybe.withDefault ""

        firstManifold : String
        firstManifold =
            property.manifolds |> List.head |> Maybe.map .id |> Maybe.withDefault ""

        firstEmitter : String
        firstEmitter =
            catalog.emitters |> List.head |> Maybe.map .id |> Maybe.withDefault ""

        firstSoil : String
        firstSoil =
            catalog.soilTypes |> List.head |> Maybe.map .id |> Maybe.withDefault ""
    in
    case existing of
        Just z ->
            { mode = mode
            , id = z.id
            , yardId = z.yardId
            , manifoldId = z.manifoldId
            , plantKind = z.plantKind
            , emitterSpecId = z.emitterSpecId
            , soilTypeId = z.soilTypeId
            , areaSqFt = String.fromFloat z.areaSqFt
            , notes = Maybe.withDefault "" z.notes
            , validationError = Nothing
            }

        Nothing ->
            { mode = mode
            , id = ""
            , yardId = firstYard
            , manifoldId = firstManifold
            , plantKind = "veggie-bed"
            , emitterSpecId = firstEmitter
            , soilTypeId = firstSoil
            , areaSqFt = "50"
            , notes = ""
            , validationError = Nothing
            }


type Msg
    = SetId String
    | SetYardId String
    | SetManifoldId String
    | SetPlantKind String
    | SetEmitterSpecId String
    | SetSoilTypeId String
    | SetAreaSqFt String
    | SetNotes String


update : Msg -> Form -> Form
update msg f =
    case msg of
        SetId v ->
            { f | id = v, validationError = Nothing }

        SetYardId v ->
            { f | yardId = v }

        SetManifoldId v ->
            { f | manifoldId = v }

        SetPlantKind v ->
            { f | plantKind = v }

        SetEmitterSpecId v ->
            { f | emitterSpecId = v }

        SetSoilTypeId v ->
            { f | soilTypeId = v }

        SetAreaSqFt v ->
            { f | areaSqFt = v, validationError = Nothing }

        SetNotes v ->
            { f | notes = v }


{-| Try to extract typed CRUD requests from the form's current
state. For `Create` mode produces a `ZoneCreateRequest`; for
`Edit` mode produces a partial `ZoneUpdateRequest` carrying every
field (the API tolerates full bodies). Returns `Err message`
when a required field is blank or the area can't parse.
-}
submit : Form -> Result String (Result Api.ZoneCreateRequest ( String, Api.ZoneUpdateRequest ))
submit f =
    if f.id |> String.trim |> String.isEmpty then
        Err "Zone id is required."

    else
        case String.toFloat (String.trim f.areaSqFt) of
            Nothing ->
                Err "Area must be a number."

            Just area ->
                if not (area > 0) then
                    Err "Area must be greater than zero."

                else
                    let
                        notesMaybe : Maybe String
                        notesMaybe =
                            if String.isEmpty (String.trim f.notes) then
                                Nothing

                            else
                                Just f.notes
                    in
                    case f.mode of
                        Create ->
                            Ok
                                (Err
                                    { id = f.id
                                    , yardId = f.yardId
                                    , manifoldId = f.manifoldId
                                    , plantKind = f.plantKind
                                    , emitterSpecId = f.emitterSpecId
                                    , soilTypeId = f.soilTypeId
                                    , areaSqFt = area
                                    , notes = notesMaybe
                                    }
                                )

                        Edit existingId ->
                            Ok
                                (Ok
                                    ( existingId
                                    , { yardId = Just f.yardId
                                      , manifoldId = Just f.manifoldId
                                      , plantKind = Just f.plantKind
                                      , emitterSpecId = Just f.emitterSpecId
                                      , soilTypeId = Just f.soilTypeId
                                      , areaSqFt = Just area
                                      , notes = notesMaybe
                                      }
                                    )
                                )


view :
    { form : Form
    , property : Api.Property
    , catalog : Api.Catalog
    , onMsg : Msg -> msg
    , onSubmit : msg
    , onCancel : msg
    , isBusy : Bool
    }
    -> Html msg
view args =
    let
        f =
            args.form

        title : String
        title =
            case f.mode of
                Create ->
                    "Add zone"

                Edit zid ->
                    "Edit zone — " ++ zid

        idField : Html msg
        idField =
            case f.mode of
                Create ->
                    formRow "Zone id"
                        (input
                            [ Attr.type_ "text"
                            , Attr.value f.id
                            , Attr.placeholder "e.g. zone-c1-shrubs"
                            , Attr.required True
                            , onInput (args.onMsg << SetId)
                            ]
                            []
                        )

                Edit _ ->
                    -- Editing the id would conflict with the URL contract;
                    -- show it but don't let the user change it.
                    formRow "Zone id"
                        (input
                            [ Attr.type_ "text"
                            , Attr.value f.id
                            , Attr.disabled True
                            ]
                            []
                        )

        yardOptions : List (Html msg)
        yardOptions =
            args.property.yards
                |> List.map
                    (\y ->
                        option
                            [ Attr.value y.id
                            , Attr.selected (y.id == f.yardId)
                            ]
                            [ text (y.name ++ " — " ++ y.id) ]
                    )

        manifoldOptions : List (Html msg)
        manifoldOptions =
            args.property.manifolds
                |> List.map
                    (\m ->
                        option
                            [ Attr.value m.id
                            , Attr.selected (m.id == f.manifoldId)
                            ]
                            [ text (m.id ++ " (" ++ m.modelId ++ ")") ]
                    )

        plantKindOptions : List (Html msg)
        plantKindOptions =
            [ ( "veggie-bed", "Veggie bed" )
            , ( "shrub", "Shrub" )
            , ( "perennial", "Perennial" )
            , ( "tree", "Tree" )
            ]
                |> List.map
                    (\( v, label_ ) ->
                        option
                            [ Attr.value v
                            , Attr.selected (v == f.plantKind)
                            ]
                            [ text label_ ]
                    )

        emitterOptions : List (Html msg)
        emitterOptions =
            args.catalog.emitters
                |> List.map
                    (\e ->
                        option
                            [ Attr.value e.id
                            , Attr.selected (e.id == f.emitterSpecId)
                            ]
                            [ text
                                (e.name
                                    ++ " · "
                                    ++ String.fromFloat e.flowGph
                                    ++ " GPH"
                                )
                            ]
                    )

        soilOptions : List (Html msg)
        soilOptions =
            args.catalog.soilTypes
                |> List.map
                    (\s ->
                        option
                            [ Attr.value s.id
                            , Attr.selected (s.id == f.soilTypeId)
                            ]
                            [ text (s.name ++ " (" ++ s.id ++ ")") ]
                    )

        errorBanner : Html msg
        errorBanner =
            case f.validationError of
                Just msg ->
                    div [ Attr.class "editor-error" ] [ text ("⚠ " ++ msg) ]

                Nothing ->
                    text ""

        submitLabel : String
        submitLabel =
            case f.mode of
                Create ->
                    "Add zone"

                Edit _ ->
                    "Save changes"
    in
    div [ Attr.class "modal-backdrop" ]
        [ div [ Attr.class "modal" ]
            [ h2 [] [ text title ]
            , errorBanner
            , form [ onSubmit args.onSubmit, Attr.class "zone-form" ]
                [ idField
                , formRow "Yard"
                    (select [ onInput (args.onMsg << SetYardId) ] yardOptions)
                , formRow "Manifold"
                    (select [ onInput (args.onMsg << SetManifoldId) ] manifoldOptions)
                , formRow "Plant kind"
                    (select [ onInput (args.onMsg << SetPlantKind) ] plantKindOptions)
                , formRow "Emitter"
                    (select [ onInput (args.onMsg << SetEmitterSpecId) ] emitterOptions)
                , formRow "Soil type"
                    (select [ onInput (args.onMsg << SetSoilTypeId) ] soilOptions)
                , formRow "Area (sq ft)"
                    (input
                        [ Attr.type_ "number"
                        , Attr.value f.areaSqFt
                        , Attr.step "any"
                        , Attr.min "0.1"
                        , onInput (args.onMsg << SetAreaSqFt)
                        ]
                        []
                    )
                , formRow "Notes"
                    (textarea
                        [ Attr.value f.notes
                        , Attr.placeholder "optional"
                        , Attr.rows 2
                        , onInput (args.onMsg << SetNotes)
                        ]
                        []
                    )
                , div [ Attr.class "form-actions" ]
                    [ button
                        [ Attr.type_ "button"
                        , Attr.class "btn"
                        , onClick args.onCancel
                        ]
                        [ text "Cancel" ]
                    , button
                        [ Attr.type_ "submit"
                        , Attr.class "btn btn-run"
                        , Attr.disabled args.isBusy
                        ]
                        [ text submitLabel ]
                    ]
                ]
            ]
        ]


formRow : String -> Html msg -> Html msg
formRow labelText control =
    div [ Attr.class "form-row" ]
        [ label [] [ text labelText ]
        , control
        ]
