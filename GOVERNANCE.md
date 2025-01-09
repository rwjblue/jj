# Jujutsu Governance

## Overview

Jujutsu is an open source project, led, maintained and designed for a worldwide
community. Anyone who is interested can join, contribute, and participate in the
decision-making process. This document is intended to help you understand how
you can do that.

## Project roles

There are two broadly defined roles in the Jujutsu project: **Maintainers** and
**Contributors**.

### Maintainers

**Maintainers** are the people who contribute, review, guide, and collectively
make decisions about the direction and scope of the project (see:
[Decision Making](#decision-making)). Maintainers are elected by a
[voting process](#adding-and-removing-maintainers).

A typical Maintainer is not only someone who has made "large" contributions, but
someone who has shown they are continuously committed to the project and its
community. Some expected responsibilities of maintainers include (but are not
exclusively limited to):

- Displaying a high level of commitment to the project and its community, and
  being a role model for others.
- Writing patches &mdash; a lot of patches, especially "glue code" or "grunt
  work" or general "housekeeping"; fixing bugs, ensuring documentation is always
  high quality, consistent UX design, improving processes, making judgments on
  dependencies, handling security vulnerabilities, and so on and so forth.
- Reviewing code submitted by others &mdash; with an eye to maintainability,
  performance, code quality, and "style" (fitting in with the project).
- Participating in design discussions, especially with regards to architecture
  or long-term vision.

This is not an exhaustive list, nor is it intended that every Maintainer does
each and every one of these individual tasks to equal amounts. Rather this is
only a guideline for what Maintainers are expected to conceptually do.

In short, Maintainers are the outwardly visible stewards of the project.

#### Current list of Maintainers

The current list of Maintainers:

- Austin Seipp (@thoughtpolice)
- Ilya Grigoriev (@ilyagr)
- Martin von Zweigbergk (@martinvonz)
- Waleed Khan (@arxanas)
- Yuya Nishihara (@yuja)

### Contributors

We consider contributors to be active participants in the project and community
who are _not_ maintainers. These are people who might:

- Help users by answering questions
- Participating in lively and respectful discussions across various channels
- Submit high-quality bug reports
- Submit patches or pull requests
- Help with testing and quality assurance
- Submit feedback about planned features, use cases, or bugs

We essentially define them as **people who actively participate in the
project**. Examples of things that would _not_ make you a contributor are:

- Submitting a single bug report and never returning
- Writing blog posts or other evangelism
- Using the software in production
- Forking the project and maintaining your own version
- Writing a third-party tool or add-on

While these are all generally quite valuable, we don't consider these
ongoing contributions to the codebase or project itself, and on their own do not
constitute "active participation".

Contributors and their input are valued and their input is expected to be taken
and respected by Maintainers; not every discussion will result in a change, but
it is intended that every voice will be heard.

## Processes

For the purposes of making decisions across the project, the following processes
are defined.

### Decision-Making

The person proposing a decision to be made (i.e. technical, project direction,
etc.) can offer a proposal, along with a 2-to-4 week deadline for discussion.
During this time, Maintainers may participate with a vote of:

A) Support B) Reject C) Abstain

Each Maintainer gets one vote. The total number of "participating votes" is the
number of Maintainer votes which are not Abstain. The proposal is accepted when
more than half of the participating votes are Support.

In the event that a decision is reached before the proposed timeline, said
proposal can move on and be accepted immediately. In the event no consensus is
reached, a proposal may be re-submitted later on.

This document itself is subject to the Decision-Making process by the existing
set of Maintainers.

### Adding and Removing Maintainers

An active Contributor may, at any given time, nominate themselves or another
Contributor to become a Maintainer. This process is purely optional and no
Contributor is expected to do so; however, self-nomination is encouraged for
active participants. A vote and discussion by the existing Maintainers will be
used to decide the outcome.

Note that Contributors should demonstrate a high standard of continuous
participation to become a Maintainer; the upper limit on the number of
Maintainers is practically bounded, and so rejection should be considered as a
real possibility. As the scope of the project changes, this limit may increase,
but it is fundamentally fluid. (If you are unsure, you are free to privately ask
existing Maintainers before self-nominating if there is room.)

A Maintainer may, at any time, cede their responsibility and step down without a
vote.

A Maintainer can be removed by other Maintainers, subject to a vote of at-least
a 2/3rds majority from the existing Maintainer group (excluding the vote of the
Maintainer in question). This can be due to lack of participation, conduct
violations, etc. Note that Maintainers are subject to a higher set of behavioral
and communicative standards than average contributor or participant.
