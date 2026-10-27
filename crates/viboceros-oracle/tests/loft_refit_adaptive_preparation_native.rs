//! Actual command/SDK preparation across the saved reference corpus.
#[path = "support/loft_refit_admission.rs"]
mod admission;
macro_rules! families { ($($name:ident,$family:literal;)*) => { $(
    #[test] fn $name() {
        let count=admission::read($family,"fixtures")["operations"].as_array().unwrap().len();
        admission::compare($family,0..count);
    }
)*}; }
families! {
    coarse,"loft_refit_coarse";
    candidates,"loft_refit_candidates";
    candidate_boundary,"loft_refit_candidate_boundary";
    adjustment,"loft_refit_adjustment";
    adjustment_fit_clock,"loft_refit_adjustment_fit_clock";
    adjustment_ellipses,"loft_refit_adjustment_ellipses";
    adjustment_local,"loft_refit_adjustment_local";
    adjustment_commands,"loft_refit_adjustment_commands";
    polylines,"loft_refit_polylines";
    polyline_angles,"loft_refit_polyline_angles";
    polyline_ends,"loft_refit_polyline_ends";
    lines,"loft_refit_lines";
    multispan,"loft_refit_multispan";
    initial_stations,"loft_refit_initial_stations";
    initial_holdouts,"loft_refit_initial_holdouts";
    adaptive_curved,"loft_refit_adaptive_curved";
    curved_clocks,"loft_refit_curved_clocks";
    join_scales,"loft_refit_join_scales";
    join_curvature,"loft_refit_join_curvature";
    unequal_sampling,"loft_refit_unequal_sampling";
    stage_reserves,"loft_refit_stage_reserves";
    source_continuity,"loft_refit_source_continuity";
    arc_clocks,"loft_refit_arc_clocks";
    angles,"loft_refit_angles";
    angle_holdouts,"loft_refit_angle_holdouts";
    shallow_angles,"loft_refit_shallow_angles";
    shallow_holdouts,"loft_refit_shallow_holdouts";
    corner_boundary,"loft_refit_corner_boundary";
    corner_suppression,"loft_refit_corner_suppression";
    suppressed_initial,"loft_refit_suppressed_initial";
    matched_probe_angles,"loft_refit_matched_probe_angles";
    subdivision,"loft_refit_subdivision";
}

// Keep independent expensive certificate groups concurrent in either backend.
macro_rules! ranges { ($($name:ident,$family:literal,$range:expr;)*) => { $(
    #[test] fn $name() { admission::compare($family,$range); }
)*}; }
ranges! {
    ordinary_and_lines,"loft_refit",0..25;
    fine_quarter_normal,"loft_refit",25..26;
    fine_quarter_loose,"loft_refit",26..27;
    fine_quarter_tight,"loft_refit",27..28;
    fine_quarter_straight,"loft_refit",28..29;
    fine_quarter_uniform,"loft_refit",29..30;
    quarter_coarser,"loft_refit",30..40;
    cubic_ordinary,"loft_refit",40..45;
    fine_cubic_normal,"loft_refit",45..46;
    fine_cubic_loose,"loft_refit",46..47;
    fine_cubic_tight,"loft_refit",47..48;
    fine_cubic_straight,"loft_refit",48..49;
    fine_cubic_uniform,"loft_refit",49..50;
    cubic_coarser,"loft_refit",50..60;
    circles_and_polygons,"loft_refit",60..100;
    smooth_shapes,"loft_refit_adaptive_shapes",0..3;
    rational_shapes,"loft_refit_adaptive_shapes",3..6;
    spatial_shapes,"loft_refit_adaptive_shapes",6..9;
    smooth_join_modes,"loft_refit_join_modes",0..4;
    rational_join_modes,"loft_refit_join_modes",4..8;
    spatial_join_modes,"loft_refit_join_modes",8..12;
    quarter_admission,"loft_refit_adaptive_admission",0..12;
    cubic_admission,"loft_refit_adaptive_admission",12..24;
    smooth_cubic_admission,"loft_refit_adaptive_admission",24..36;
    quintic_admission,"loft_refit_adaptive_admission",36..48;
}

// Each expensive source/context is an independent test, keeping available CPU
// cores busy when the short families finish.
macro_rules! cases { ($family:literal; $($name:ident=$index:literal;)*) => { $(
    #[test] fn $name() { admission::compare($family,$index..$index+1); }
)*}; }
cases! { "loft_refit_adaptive_contexts";
    context_0=0;
    context_1=1;
    context_2=2;
    context_3=3;
    context_4=4;
    context_5=5;
    context_6=6;
    context_7=7;
    context_8=8;
    context_9=9;
    context_10=10;
    context_11=11;
    context_12=12;
    context_13=13;
    context_14=14;
    context_15=15;
    context_16=16;
    context_17=17;
    context_18=18;
    context_19=19;
    context_20=20;
    context_21=21;
    context_22=22;
    context_23=23;
    context_24=24;
    context_25=25;
    context_26=26;
    context_27=27;
    context_28=28;
    context_29=29;
    context_30=30;
    context_31=31;
    context_32=32;
    context_33=33;
    context_34=34;
    context_35=35;
    context_36=36;
    context_37=37;
    context_38=38;
    context_39=39;
    context_40=40;
    context_41=41;
    context_42=42;
    context_43=43;
    context_44=44;
    context_45=45;
    context_46=46;
    context_47=47;
    context_48=48;
    context_49=49;
    context_50=50;
    context_51=51;
    context_52=52;
    context_53=53;
    context_54=54;
    context_55=55;
    context_56=56;
    context_57=57;
    context_58=58;
    context_59=59;
    context_60=60;
    context_61=61;
    context_62=62;
    context_63=63;
}
cases! { "loft_refit_adaptive_shapes";
    quintic_shape_9=9;
    quintic_shape_10=10;
    quintic_shape_11=11;
}
cases! { "loft_refit_join_modes";
    quintic_join_12=12;
    quintic_join_13=13;
    quintic_join_14=14;
    quintic_join_15=15;
}
