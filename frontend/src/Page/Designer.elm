module Page.Designer exposing (Model, Msg, init, update, view)

{-| Property designer: a form-driven page that collects a
`PlanRequest` (property name, climate zone, budget, preferences,
and a list of yards with their zones) and, on submit, POSTs to
`/api/plan` and renders the ranked candidate plans side by side.

The form is intentionally additive: start with one yard and one
zone; the user clicks "+ Add yard" / "+ Add zone" as needed.
Each row has a "remove" button so iterating is cheap.

-}

import Api
import Html
    exposing
        ( Html
        , button
        , details
        , div
        , fieldset
        , form
        , h1
        , h2
        , h3
        , input
        , label
        , legend
        , li
        , option
        , p
        , section
        , select
        , span
        , summary
        , table
        , tbody
        , td
        , text
        , th
        , thead
        , tr
        , ul
        )
import Html.Attributes as Attr
import Html.Events exposing (onClick, onInput, onSubmit)
import Http



-- ── Model ─────────────────────────────────────────────────────────────────


type alias Model =
    { form : FormState
    , plans : PlanStatus
    }


type alias FormState =
    { propertyId : String
    , propertyName : String
    , climateZone : String
    , budgetUsd : String
    , preferSmartController : Bool
    , requirePressureCompensating : Bool
    , soilTypeId : String
    , topN : String
    , yards : List YardForm
    }


type alias YardForm =
    { id : String
    , name : String
    , areaSqFt : String
    , mainsPressurePsi : String
    , zones : List ZoneForm
    }


type alias ZoneForm =
    { nameSuffix : String
    , plantKind : String
    , areaSqFt : String
    }


type PlanStatus
    = Idle
    | Planning
    | PlansLoaded (List Api.Plan)
    | PlansFailed String


init : ( Model, Cmd Msg )
init =
    ( { form = initialForm
      , plans = Idle
      }
    , Cmd.none
    )


initialForm : FormState
initialForm =
    { propertyId = "new-property"
    , propertyName = "New Property"
    , climateZone = "portland-or"
    , budgetUsd = "1500"
    , preferSmartController = True
    , requirePressureCompensating = False
    , soilTypeId = "silty-clay-loam"
    , topN = "3"
    , yards =
        [ { id = "yard-a"
          , name = "Yard A"
          , areaSqFt = "800"
          , mainsPressurePsi = "60"
          , zones =
                [ { nameSuffix = "veggies"
                  , plantKind = "veggie-bed"
                  , areaSqFt = "100"
                  }
                , { nameSuffix = "shrubs"
                  , plantKind = "shrub"
                  , areaSqFt = "200"
                  }
                ]
          }
        ]
    }



-- ── Messages ──────────────────────────────────────────────────────────────


type Msg
    = SetPropertyId String
    | SetPropertyName String
    | SetClimateZone String
    | SetBudgetUsd String
    | TogglePreferSmart
    | ToggleRequirePc
    | SetSoilTypeId String
    | SetTopN String
    | AddYard
    | RemoveYard Int
    | SetYardField Int YardField String
    | AddZone Int
    | RemoveZone Int Int
    | SetZoneField Int Int ZoneField String
    | SubmitForm
    | GotPlans (Result Http.Error Api.PlanResponse)


type YardField
    = YardId
    | YardName
    | YardArea
    | YardMainsPsi


type ZoneField
    = ZoneNameSuffix
    | ZonePlantKind
    | ZoneArea



-- ── Update ────────────────────────────────────────────────────────────────


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        SetPropertyId v ->
            ( { model | form = setFormField (\f -> { f | propertyId = v }) model.form }, Cmd.none )

        SetPropertyName v ->
            ( { model | form = setFormField (\f -> { f | propertyName = v }) model.form }, Cmd.none )

        SetClimateZone v ->
            ( { model | form = setFormField (\f -> { f | climateZone = v }) model.form }, Cmd.none )

        SetBudgetUsd v ->
            ( { model | form = setFormField (\f -> { f | budgetUsd = v }) model.form }, Cmd.none )

        TogglePreferSmart ->
            ( { model | form = setFormField (\f -> { f | preferSmartController = not f.preferSmartController }) model.form }, Cmd.none )

        ToggleRequirePc ->
            ( { model | form = setFormField (\f -> { f | requirePressureCompensating = not f.requirePressureCompensating }) model.form }, Cmd.none )

        SetSoilTypeId v ->
            ( { model | form = setFormField (\f -> { f | soilTypeId = v }) model.form }, Cmd.none )

        SetTopN v ->
            ( { model | form = setFormField (\f -> { f | topN = v }) model.form }, Cmd.none )

        AddYard ->
            let
                newYard : YardForm
                newYard =
                    { id = "yard-" ++ String.fromInt (List.length model.form.yards + 1)
                    , name = "New Yard"
                    , areaSqFt = "400"
                    , mainsPressurePsi = "60"
                    , zones =
                        [ { nameSuffix = "zone-1"
                          , plantKind = "shrub"
                          , areaSqFt = "100"
                          }
                        ]
                    }
            in
            ( { model | form = setFormField (\f -> { f | yards = f.yards ++ [ newYard ] }) model.form }, Cmd.none )

        RemoveYard idx ->
            ( { model | form = setFormField (\f -> { f | yards = removeAt idx f.yards }) model.form }, Cmd.none )

        SetYardField idx field v ->
            let
                update_ : YardForm -> YardForm
                update_ y =
                    case field of
                        YardId ->
                            { y | id = v }

                        YardName ->
                            { y | name = v }

                        YardArea ->
                            { y | areaSqFt = v }

                        YardMainsPsi ->
                            { y | mainsPressurePsi = v }
            in
            ( { model | form = setFormField (\f -> { f | yards = updateAt idx update_ f.yards }) model.form }, Cmd.none )

        AddZone yardIdx ->
            let
                newZone : ZoneForm
                newZone =
                    { nameSuffix = "zone-new"
                    , plantKind = "shrub"
                    , areaSqFt = "100"
                    }
            in
            ( { model
                | form =
                    setFormField
                        (\f ->
                            { f
                                | yards =
                                    updateAt yardIdx
                                        (\y -> { y | zones = y.zones ++ [ newZone ] })
                                        f.yards
                            }
                        )
                        model.form
              }
            , Cmd.none
            )

        RemoveZone yardIdx zoneIdx ->
            ( { model
                | form =
                    setFormField
                        (\f ->
                            { f
                                | yards =
                                    updateAt yardIdx
                                        (\y -> { y | zones = removeAt zoneIdx y.zones })
                                        f.yards
                            }
                        )
                        model.form
              }
            , Cmd.none
            )

        SetZoneField yardIdx zoneIdx field v ->
            let
                update_ : ZoneForm -> ZoneForm
                update_ z =
                    case field of
                        ZoneNameSuffix ->
                            { z | nameSuffix = v }

                        ZonePlantKind ->
                            { z | plantKind = v }

                        ZoneArea ->
                            { z | areaSqFt = v }
            in
            ( { model
                | form =
                    setFormField
                        (\f ->
                            { f
                                | yards =
                                    updateAt yardIdx
                                        (\y -> { y | zones = updateAt zoneIdx update_ y.zones })
                                        f.yards
                            }
                        )
                        model.form
              }
            , Cmd.none
            )

        SubmitForm ->
            case toPlanRequest model.form of
                Ok req ->
                    ( { model | plans = Planning }, Api.postPlan req GotPlans )

                Err errMsg ->
                    ( { model | plans = PlansFailed errMsg }, Cmd.none )

        GotPlans (Ok resp) ->
            ( { model | plans = PlansLoaded resp.plans }, Cmd.none )

        GotPlans (Err err) ->
            ( { model | plans = PlansFailed (httpErrToString err) }, Cmd.none )


setFormField : (FormState -> FormState) -> FormState -> FormState
setFormField f s =
    f s


removeAt : Int -> List a -> List a
removeAt idx xs =
    List.indexedMap Tuple.pair xs
        |> List.filter (\( i, _ ) -> i /= idx)
        |> List.map Tuple.second


updateAt : Int -> (a -> a) -> List a -> List a
updateAt idx f xs =
    List.indexedMap
        (\i x ->
            if i == idx then
                f x

            else
                x
        )
        xs



-- ── Form → PlanRequest ────────────────────────────────────────────────────


toPlanRequest : FormState -> Result String Api.PlanRequest
toPlanRequest f =
    let
        parseFloat : String -> String -> Result String Float
        parseFloat label_ v =
            case String.toFloat v of
                Just n ->
                    Ok n

                Nothing ->
                    Err ("Expected a number for " ++ label_ ++ ", got " ++ "\"" ++ v ++ "\"")

        parseInt : String -> String -> Result String Int
        parseInt label_ v =
            case String.toInt v of
                Just n ->
                    Ok n

                Nothing ->
                    Err ("Expected an integer for " ++ label_ ++ ", got " ++ "\"" ++ v ++ "\"")

        budgetResult : Result String (Maybe Float)
        budgetResult =
            if String.trim f.budgetUsd == "" then
                Ok Nothing

            else
                parseFloat "budget" f.budgetUsd |> Result.map Just

        yardsResult : Result String (List Api.PlanYardInput)
        yardsResult =
            resultListMap (yardToApi parseFloat) f.yards

        topNResult : Result String Int
        topNResult =
            parseInt "top_n" f.topN
    in
    Result.map3
        (\budget yards topN ->
            { propertyId = f.propertyId
            , propertyName = f.propertyName
            , climateZone = f.climateZone
            , yards = yards
            , budgetUsd = budget
            , preferSmartController = f.preferSmartController
            , requirePressureCompensating = f.requirePressureCompensating
            , soilTypeId = f.soilTypeId
            , topN = topN
            }
        )
        budgetResult
        yardsResult
        topNResult


yardToApi : (String -> String -> Result String Float) -> YardForm -> Result String Api.PlanYardInput
yardToApi parseFloat y =
    Result.map3
        (\area psi zones ->
            { id = y.id
            , name = y.name
            , areaSqFt = area
            , mainsPressurePsi = psi
            , zones = zones
            }
        )
        (parseFloat "yard area" y.areaSqFt)
        (parseFloat "mains pressure" y.mainsPressurePsi)
        (resultListMap (zoneToApi parseFloat) y.zones)


zoneToApi : (String -> String -> Result String Float) -> ZoneForm -> Result String Api.PlanZoneInput
zoneToApi parseFloat z =
    parseFloat "zone area" z.areaSqFt
        |> Result.map
            (\area ->
                { nameSuffix = z.nameSuffix
                , plantKind = z.plantKind
                , areaSqFt = area
                }
            )


resultListMap : (a -> Result e b) -> List a -> Result e (List b)
resultListMap f xs =
    List.foldr
        (\x acc ->
            case ( f x, acc ) of
                ( Ok b, Ok bs ) ->
                    Ok (b :: bs)

                ( Err e, _ ) ->
                    Err e

                ( _, Err e ) ->
                    Err e
        )
        (Ok [])
        xs


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
    div [ Attr.class "designer-page" ]
        [ h1 [] [ text "Designer" ]
        , p [ Attr.class "designer-intro" ]
            [ text
                "Sketch a property, set a budget, and see catalog-driven "
            , text "hardware plans ranked by fit."
            ]
        , section [ Attr.class "designer-form" ] [ viewForm model.form ]
        , section [ Attr.class "designer-plans" ] [ viewPlans model.plans ]
        ]


viewForm : FormState -> Html Msg
viewForm f =
    form [ onSubmit SubmitForm, Attr.class "plan-form" ]
        [ fieldset []
            [ legend [] [ text "Property" ]
            , labelled "Property id" (textInput f.propertyId SetPropertyId)
            , labelled "Property name" (textInput f.propertyName SetPropertyName)
            , labelled "Climate zone" (textInput f.climateZone SetClimateZone)
            , labelled "Budget (USD, leave blank for none)" (textInput f.budgetUsd SetBudgetUsd)
            , labelled "Soil type id" (textInput f.soilTypeId SetSoilTypeId)
            , labelled "Top N plans" (textInput f.topN SetTopN)
            , label [ Attr.class "form-toggle" ]
                [ input
                    [ Attr.type_ "checkbox"
                    , Attr.checked f.preferSmartController
                    , onClick TogglePreferSmart
                    ]
                    []
                , text " Prefer smart (Wi-Fi) controller"
                ]
            , label [ Attr.class "form-toggle" ]
                [ input
                    [ Attr.type_ "checkbox"
                    , Attr.checked f.requirePressureCompensating
                    , onClick ToggleRequirePc
                    ]
                    []
                , text " Require pressure-compensating emitters"
                ]
            ]
        , fieldset []
            [ legend [] [ text "Yards" ]
            , div [] (List.indexedMap viewYard f.yards)
            , button
                [ Attr.type_ "button"
                , Attr.class "btn btn-add"
                , onClick AddYard
                ]
                [ text "+ Add yard" ]
            ]
        , div [ Attr.class "form-actions" ]
            [ button
                [ Attr.type_ "submit"
                , Attr.class "btn btn-primary"
                ]
                [ text "Plan" ]
            ]
        ]


viewYard : Int -> YardForm -> Html Msg
viewYard idx y =
    div [ Attr.class "yard-block" ]
        [ div [ Attr.class "yard-head" ]
            [ h3 [] [ text (y.name ++ " (" ++ y.id ++ ")") ]
            , button
                [ Attr.type_ "button"
                , Attr.class "btn btn-remove"
                , onClick (RemoveYard idx)
                ]
                [ text "Remove yard" ]
            ]
        , labelled "Yard id" (textInput y.id (SetYardField idx YardId))
        , labelled "Yard name" (textInput y.name (SetYardField idx YardName))
        , labelled "Area (sq ft)" (textInput y.areaSqFt (SetYardField idx YardArea))
        , labelled "Mains pressure (psi)" (textInput y.mainsPressurePsi (SetYardField idx YardMainsPsi))
        , div [ Attr.class "zones-sub" ]
            [ h3 [] [ text "Zones" ]
            , div [] (List.indexedMap (viewZone idx) y.zones)
            , button
                [ Attr.type_ "button"
                , Attr.class "btn btn-add"
                , onClick (AddZone idx)
                ]
                [ text "+ Add zone" ]
            ]
        ]


viewZone : Int -> Int -> ZoneForm -> Html Msg
viewZone yardIdx zoneIdx z =
    div [ Attr.class "zone-block" ]
        [ labelled "Name suffix" (textInput z.nameSuffix (SetZoneField yardIdx zoneIdx ZoneNameSuffix))
        , labelled "Plant kind"
            (select
                [ onInput (SetZoneField yardIdx zoneIdx ZonePlantKind) ]
                (List.map (plantOption z.plantKind)
                    [ ( "veggie-bed", "Veggie bed" )
                    , ( "shrub", "Shrub" )
                    , ( "perennial", "Perennial" )
                    , ( "tree", "Tree" )
                    ]
                )
            )
        , labelled "Area (sq ft)" (textInput z.areaSqFt (SetZoneField yardIdx zoneIdx ZoneArea))
        , button
            [ Attr.type_ "button"
            , Attr.class "btn btn-remove"
            , onClick (RemoveZone yardIdx zoneIdx)
            ]
            [ text "Remove zone" ]
        ]


plantOption : String -> ( String, String ) -> Html Msg
plantOption selected ( value, label_ ) =
    option
        [ Attr.value value
        , Attr.selected (selected == value)
        ]
        [ text label_ ]


labelled : String -> Html Msg -> Html Msg
labelled lbl inner =
    label [ Attr.class "form-field" ]
        [ span [ Attr.class "form-field-label" ] [ text lbl ]
        , inner
        ]


textInput : String -> (String -> Msg) -> Html Msg
textInput v msg =
    input
        [ Attr.type_ "text"
        , Attr.value v
        , onInput msg
        ]
        []


viewPlans : PlanStatus -> Html Msg
viewPlans status =
    case status of
        Idle ->
            p [ Attr.class "plans-idle" ]
                [ text "Submit the form to see candidate plans." ]

        Planning ->
            p [ Attr.class "plans-loading" ] [ text "Planning…" ]

        PlansFailed m ->
            div [ Attr.class "plans-failed" ]
                [ h2 [] [ text "Planner error" ]
                , p [] [ text m ]
                ]

        PlansLoaded [] ->
            p [] [ text "No plans returned." ]

        PlansLoaded plans ->
            div [ Attr.class "plans-grid" ]
                (h2 [] [ text "Candidate plans" ]
                    :: List.map viewPlanCard plans
                )


viewPlanCard : Api.Plan -> Html Msg
viewPlanCard plan =
    div [ Attr.class "plan-card" ]
        [ h3 [] [ text plan.planId ]
        , p [ Attr.class "plan-controller" ]
            [ text ("Controller: " ++ plan.controllerModelId)
            , text (" (up to " ++ String.fromInt plan.controllerMaxZones ++ " zones)")
            ]
        , p [ Attr.class "plan-score" ]
            [ text ("Score: " ++ toFixed 1 plan.score) ]
        , p [ Attr.class "plan-total" ]
            [ text ("Total: $" ++ toFixed 2 plan.bom.totalUsd) ]
        , details []
            [ summary [] [ text "Rationale" ]
            , ul [] (List.map (\r -> li [] [ text r ]) plan.rationale)
            ]
        , details []
            [ summary [] [ text ("Bill of materials (" ++ String.fromInt (List.length plan.bom.lines) ++ " lines)") ]
            , viewBom plan.bom
            ]
        ]


viewBom : Api.PlanBom -> Html Msg
viewBom bom =
    table [ Attr.class "bom-table" ]
        [ thead []
            [ tr []
                [ th [] [ text "Category" ]
                , th [] [ text "Item" ]
                , th [] [ text "Qty" ]
                , th [] [ text "Unit $" ]
                , th [] [ text "Line $" ]
                ]
            ]
        , tbody [] (List.map viewBomRow bom.lines)
        ]


viewBomRow : Api.BomLine -> Html Msg
viewBomRow l =
    tr []
        [ td [] [ text l.category ]
        , td [] [ text l.displayName ]
        , td [] [ text (String.fromInt l.quantity) ]
        , td [] [ text (toFixed 2 l.unitPriceUsd) ]
        , td [] [ text (toFixed 2 l.lineTotalUsd) ]
        ]


toFixed : Int -> Float -> String
toFixed places value =
    let
        scale : Float
        scale =
            10.0 ^ toFloat places

        scaled : Int
        scaled =
            round (value * scale)

        intPart : Int
        intPart =
            scaled // round scale

        fracPart : Int
        fracPart =
            modBy (round scale) scaled
    in
    if places == 0 then
        String.fromInt scaled

    else
        String.fromInt intPart
            ++ "."
            ++ String.padLeft places '0' (String.fromInt fracPart)
