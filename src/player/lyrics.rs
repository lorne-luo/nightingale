use bevy::prelude::*;

use crate::analyzer::transcript::{Segment, Transcript};
use crate::ui::UiTheme;

#[derive(Resource)]
pub struct LyricsState {
    pub transcript: Transcript,
    pub current_segment: usize,
}

#[derive(Component)]
pub struct LyricsRoot;

#[derive(Component)]
pub struct CurrentLine;

#[derive(Component)]
pub struct NextLine;

#[derive(Component)]
pub struct CountdownNode;

#[derive(Component)]
pub struct CurrentLineText;

#[derive(Component)]
pub struct NextLineText;

const COUNTDOWN_DURATION: f64 = 3.0;
const COUNTDOWN_GAP_THRESHOLD: f64 = 3.5;
const LYRICS_LEAD: f64 = 0.15;

pub fn setup_lyrics(commands: &mut Commands, transcript: &Transcript, theme: &UiTheme) {
    let state = LyricsState {
        transcript: transcript.clone(),
        current_segment: usize::MAX,
    };

    commands
        .spawn((
            LyricsRoot,
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(60.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(40.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                CurrentLine,
                Node {
                    flex_shrink: 0.0,
                    max_width: Val::Percent(100.0),
                    padding: UiRect::new(
                        Val::Px(20.0),
                        Val::Px(20.0),
                        Val::Px(10.0),
                        Val::Px(10.0),
                    ),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ))
            .with_children(|cl| {
                cl.spawn((
                    CountdownNode,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-36.0),
                        left: Val::Px(-36.0),
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Visibility::Hidden,
                    ZIndex(1),
                ))
                .with_children(|cd| {
                    cd.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.countdown_color),
                    ));
                });
            });

            root.spawn((
                NextLine,
                Node {
                    flex_shrink: 0.0,
                    max_width: Val::Percent(100.0),
                    padding: UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(6.0), Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ));
        });

    commands.insert_resource(state);
}

pub fn update_lyrics(
    mut lyrics: ResMut<LyricsState>,
    current_time: f64,
    mut current_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<CurrentLine>, Without<NextLine>, Without<CountdownNode>),
    >,
    mut next_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<NextLine>, Without<CurrentLine>, Without<CountdownNode>),
    >,
    mut countdown_query: Query<
        (&mut Visibility, &mut BackgroundColor, &Children),
        (With<CountdownNode>, Without<CurrentLine>, Without<NextLine>),
    >,
    mut countdown_text_query: Query<&mut Text>,
    mut current_text_query: Query<&mut TextColor, (With<CurrentLineText>, Without<NextLineText>)>,
    mut next_text_query: Query<&mut TextColor, (With<NextLineText>, Without<CurrentLineText>)>,
    commands: &mut Commands,
    theme: &UiTheme,
) {
    if lyrics.transcript.segments.is_empty() {
        return;
    }

    let seg_idx = find_current_segment(
        &lyrics.transcript.segments,
        current_time,
        lyrics.current_segment,
    );

    if seg_idx != lyrics.current_segment {
        lyrics.current_segment = seg_idx;
        let segments = &lyrics.transcript.segments;
        rebuild_lines(
            seg_idx,
            segments,
            &current_line_query,
            &next_line_query,
            commands,
            theme,
        );
    }

    let segments = &lyrics.transcript.segments;
    let seg = &segments[seg_idx];
    let active = current_time >= seg.start - LYRICS_LEAD && current_time <= seg.end + 0.5;

    let gap_before = if seg_idx == 0 {
        seg.start
    } else {
        seg.start - segments[seg_idx - 1].end
    };
    let time_until = seg.start - current_time;
    let show_countdown = gap_before >= COUNTDOWN_GAP_THRESHOLD
        && time_until > 0.0
        && time_until <= COUNTDOWN_DURATION;

    let show_current = active || show_countdown;

    let next_exists = seg_idx + 1 < segments.len();
    let show_next = show_current && next_exists;

    if let Ok((_, mut bg, mut vis)) = current_line_query.single_mut() {
        if show_current {
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.lyric_backdrop);
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if let Ok((_, mut bg, mut vis)) = next_line_query.single_mut() {
        if show_next {
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.lyric_backdrop_next);
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if let Ok(mut color) = current_text_query.single_mut() {
        *color = TextColor(if active {
            theme.sung_color
        } else {
            theme.unsung_color
        });
    }

    if let Ok(mut color) = next_text_query.single_mut() {
        *color = TextColor(theme.next_line_color);
    }

    if let Ok((mut vis, mut bg, children)) = countdown_query.single_mut() {
        if show_countdown {
            let n = time_until.ceil() as i32;
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.countdown_bg);
            for child in children.iter() {
                if let Ok(mut text) = countdown_text_query.get_mut(child) {
                    **text = format!("{n}");
                }
            }
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }
}

pub fn last_segment_end(lyrics: &LyricsState) -> f64 {
    lyrics
        .transcript
        .segments
        .last()
        .map(|s| s.end)
        .unwrap_or(0.0)
}

pub fn first_segment_start(lyrics: &LyricsState) -> f64 {
    lyrics
        .transcript
        .segments
        .first()
        .map(|s| s.start)
        .unwrap_or(0.0)
}

fn find_current_segment(segments: &[Segment], time: f64, hint: usize) -> usize {
    let start = if hint < segments.len() && time >= segments[hint].start - LYRICS_LEAD {
        hint
    } else {
        0
    };
    for i in start..segments.len() {
        let seg = &segments[i];
        if time < seg.end + 0.5 {
            if i + 1 < segments.len() && time >= segments[i + 1].start - LYRICS_LEAD {
                return i + 1;
            }
            return i;
        }
    }
    segments.len().saturating_sub(1)
}

fn rebuild_lines(
    idx: usize,
    segments: &[Segment],
    current_line_query: &Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<CurrentLine>, Without<NextLine>, Without<CountdownNode>),
    >,
    next_line_query: &Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<NextLine>, Without<CurrentLine>, Without<CountdownNode>),
    >,
    commands: &mut Commands,
    theme: &UiTheme,
) {
    if let Ok((entity, _, _)) = current_line_query.single() {
        commands.entity(entity).despawn_children();
        commands.entity(entity).with_children(|parent| {
            parent
                .spawn((
                    CountdownNode,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-36.0),
                        left: Val::Px(-36.0),
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Visibility::Hidden,
                    ZIndex(1),
                ))
                .with_children(|cd| {
                    cd.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.countdown_color),
                    ));
                });
            if idx < segments.len() {
                parent.spawn((
                    CurrentLineText,
                    Text::new(segments[idx].text.clone()),
                    TextFont {
                        font_size: 42.0,
                        ..default()
                    },
                    TextColor(theme.unsung_color),
                    TextLayout {
                        justify: Justify::Center,
                        linebreak: LineBreak::WordBoundary,
                    },
                ));
            }
        });
    }

    if let Ok((entity, _, _)) = next_line_query.single() {
        commands.entity(entity).despawn_children();
        let next_idx = idx + 1;
        if next_idx < segments.len() {
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    NextLineText,
                    Text::new(segments[next_idx].text.clone()),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(theme.next_line_color),
                    TextLayout {
                        justify: Justify::Center,
                        linebreak: LineBreak::WordBoundary,
                    },
                ));
            });
        }
    }
}
